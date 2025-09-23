use anyhow::Context;

use crate::process::Buffer;

use crate::{
    cli::Config,
    process::{counts::Stats, fastq::FastQRecord, align::Aligner},
};

pub(super) fn process_buffer<'a> (
    cfg: &'a Config,
    b: &Buffer,
    aligner: &mut Aligner,
    stats: &mut Stats<'a>,
    overlap_buf: &mut Vec<u8>,
    al_buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    
    let (fq1, fq2) = b.fastq();
    
    for(r1, r2) in fq1.zip(fq2) {
        let rec1 = r1?;
        let rec2 = r2?;
        process_records(rec1, rec2, stats, aligner, overlap_buf, al_buf, cfg.reference(), cfg.min_qual())?  
    }
    
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_records(
    rec1: FastQRecord,
    rec2: FastQRecord,
    stats: &mut Stats,
    aligner: &mut Aligner,
    ov_buf: &mut Vec<u8>,
    al_buf: &mut Vec<u8>,
    reference: &[u8],
    min_qual: u8,
) -> anyhow::Result<()> {
    let s1 = rec1
        .id()
        .split(|c| c.is_ascii_whitespace())
        .next()
        .with_context(|| "Missing read ID")?;
    let s2 = rec2
        .id()
        .split(|c| c.is_ascii_whitespace())
        .next()
        .with_context(|| "Missing read ID")?;
    if s1 != s2 {
        return Err(anyhow!("Mismatch between IDs of read 1 and read 2"));
    }

    // Reverse complement read 2 sequence
    let v = aligner.buf_mut();
    v.clear();
    for c in rec2.seq().iter().rev().map(|c| match c {
        b'A' | b'a' => b'T',
        b'C' | b'c' => b'G',
        b'G' | b'g' => b'C',
        b'T' | b't' => b'A',
        x => *x,
    }) {
        v.push(c)
    }

    // Set up aligner for overlapping reads
    aligner
        .set_alignment_free_ends(0, 15, 15, 0);
    
    // Align read 1 and read 2 together
    aligner
        .align_buf_as_text(rec1.seq())
        .with_context(|| "Error when aligning overlap")?;
    let cigar = aligner.wfs_aligner().cigar();

    let mut text_itr = aligner.buf().iter().zip(rec2.qual().iter().rev());
    let mut patt_itr = rec1.seq().iter().zip(rec1.qual().iter());

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
    
    // Set up for end-to-end alignment
    aligner
        .set_alignment_free_ends(0, 0, 0, 0);
    
    aligner.align(ov_buf, reference).with_context(|| "Error when aligning to reference")?;
    
    let cigar = aligner.wfs_aligner().cigar();
    let mut patt_itr = ov_buf.iter();
    let mut text_itr = reference.iter();
    al_buf.clear();
    for op in cigar.operations() {
        match *op {
            b'M' | b'X' => {
                let _ = text_itr.next().unwrap();
                let p = patt_itr.next().unwrap();
                al_buf.push(p.to_ascii_uppercase());
            }
            b'I' => {
                let _ = text_itr.next();
                al_buf.push(b' ')
            }
            b'D' => {
                let _ = patt_itr.next();
                if let Some(e) = al_buf.last_mut() {
                    *e = e.to_ascii_lowercase()
                }
            }
            _ => panic!("Unknown operation"),
        }
    }

    stats.add_obs(al_buf.as_ref());
    stats.add_len(ov_buf.len() as u32);
    
    Ok(())
}
