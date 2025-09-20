use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::Context;
use compress_io::compress::{CompressIo, Reader};

use super::{align::Aligner, counts::Stats};

use crate::cli::Config;

pub fn read_files(cfg: &Config, p: &[PathBuf], stats: &mut Stats) -> anyhow::Result<()> {
    debug!("Opening input files {p:?}");

    let rdrs = [
        FastQReader::open(p[0].as_path())?,
        FastQReader::open(p[1].as_path())?,
    ];

    read_from_rdrs(cfg, rdrs, stats)?;
    info!("Finished reading input file");
    Ok(())
}

struct FastQReader {
    rdr: BufReader<Reader>,
    buf: [String; 4],
}

impl FastQReader {
    fn open(p: &Path) -> anyhow::Result<Self> {
        let rdr = CompressIo::new()
            .path(p)
            .bufreader()
            .with_context(|| format!("Error opening input file {p:?}"))?;
        let buf = [String::new(), String::new(), String::new(), String::new()];
        Ok(Self { rdr, buf })
    }

    fn read_record<'a>(&'a mut self) -> anyhow::Result<Option<Read<'a>>> {
        let mut ix = 0;
        Ok(loop {
            let b = &mut self.buf[ix];
            if get_line(&mut self.rdr, b)? {
                if ix == 0 {
                    break None;
                } else {
                    return Err(anyhow!("fastq file truncated"));
                }
            }
            match ix {
                0 => {
                    if !b.starts_with('@') {
                        return Err(anyhow!("Bad fastq format (expected '@')"));
                    }
                }
                1 | 2 => {}
                3 => {
                    let read = Read {
                        id: &self.buf[0].trim_ascii_end().as_bytes()[1..],
                        seq: self.buf[1].trim_ascii_end().as_bytes(),
                        qual: self.buf[3].trim_ascii_end().as_bytes(),
                    };
                    break Some(read);
                }
                _ => panic!("Shouldn't get here"),
            }
            ix += 1
        })
    }
}

fn read_from_rdrs(
    cfg: &Config,
    mut rdrs: [FastQReader; 2],
    stats: &mut Stats,
) -> anyhow::Result<()> {
    let mut overlap_align = Aligner::default();
    overlap_align
        .wfs_aligner_mut()
        .set_alignment_free_ends(0, 15, 15, 0);

    let mut buf: Vec<u8> = Vec::with_capacity(cfg.reference().len());

    loop {
        let (rdr1, rdr2) = rdrs.split_at_mut(1);
        let r1 = rdr1[0].read_record()?;
        let r2 = rdr2[0].read_record()?;
        match (r1, r2) {
            (Some(rd1), Some(rd2)) => process_reads(
                rd1,
                rd2,
                stats,
                &mut overlap_align,
                &mut buf,
                cfg.reference(),
                cfg.min_qual()
            )?,
            (None, None) => break,
            _ => return Err(anyhow!("Input files of different lengths")),
        }
    }
    Ok(())
}

fn get_line<R: BufRead>(rdr: &mut R, b: &mut String) -> anyhow::Result<bool> {
    b.clear();
    rdr.read_line(b)
        .with_context(|| "Error reading from input file")
        .map(|l| l == 0)
}

struct Read<'a> {
    id: &'a [u8],
    seq: &'a [u8],
    qual: &'a [u8],
}

impl Read<'_> {
    fn check(&self) -> anyhow::Result<()> {
        if self.seq.len() != self.qual.len() {
            Err(anyhow!(
                "Unequal seq and qual lengths {} {}",
                self.seq.len(),
                self.qual.len()
            ))
        } else {
            Ok(())
        }
    }
}

fn process_reads(
    rd1: Read,
    rd2: Read,
    stats: &mut Stats,
    overlap: &mut Aligner,
    ov_buf: &mut Vec<u8>,
    reference: &[u8],
    min_qual: u8,
) -> anyhow::Result<()> {
    rd1.check()?;
    rd2.check()?;
    let s1 = rd1
        .id
        .split(|c| c.is_ascii_whitespace())
        .next()
        .with_context(|| "Missing read ID")?;
    let s2 = rd2
        .id
        .split(|c| c.is_ascii_whitespace())
        .next()
        .with_context(|| "Missing read ID")?;
    if s1 != s2 {
        return Err(anyhow!("Mismatch between IDs of read 1 and read 2"));
    }

    // Reverse complement read 2 sequence
    let v = overlap.buf_mut();
    v.clear();
    for c in rd2.seq.iter().rev().map(|c| match c {
        b'A' | b'a' => b'T',
        b'C' | b'c' => b'G',
        b'G' | b'g' => b'C',
        b'T' | b't' => b'A',
        x => *x,
    }) {
        v.push(c)
    }

    // Align read 1 and read 2 together
    overlap
        .align_buf_as_text(rd1.seq)
        .with_context(|| "Error when aligning overlap")?;
    let cigar = overlap.wfs_aligner().cigar();

    let mut text_itr = overlap.buf().iter().zip(rd2.qual.iter().rev());
    let mut patt_itr = rd1.seq.iter().zip(rd1.qual.iter());

    ov_buf.clear();
    for op in cigar.operations() {
        match *op {
            b'M' | b'X' => {
                let (t, qt) = text_itr.next().unwrap();
                let (p, qp) = patt_itr.next().unwrap();
                let (base, qual) = if t == p {
                    (*t, *(qt.max(qp)))
                } else if qt > qp {
                    (*t, qt - qp)
                } else {
                    (*p, qp - qt)
                };
                if qual.saturating_sub(33) >= min_qual {
                    ov_buf.push(base)
                } else {
                    ov_buf.push(b'N')
                }
            }
            b'I' => {
                let _ = text_itr.next();
            }
            b'D' => {
                let _ = patt_itr.next();
            }
            _ => panic!("Unknown operation"),
        }
    }
    if ov_buf.len() == reference.len() {
        stats.add_obs(ov_buf.as_ref());
    }
    stats.add_len(ov_buf.len() as u32);
    
    Ok(())
}
