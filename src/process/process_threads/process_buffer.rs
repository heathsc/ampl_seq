use anyhow::Context;

use crate::process::Buffer;

use crate::{
    cli::Config,
    process::{align::Aligner, counts::Stats, fastq::FastQRecord, view::ViewStore},
};

pub(super) fn process_buffer<'a>(
    cfg: &'a Config,
    b: &Buffer,
    aligner: &mut Aligner,
    stats: &mut Stats<'a>,
    overlap_buf: &mut Vec<u8>,
    al_buf: &mut Vec<u8>,
    mut view_data: Option<&mut ViewStore>,
) -> anyhow::Result<()> {
    let (fq1, fq2) = b.fastq();

    for (r1, r2) in fq1.zip(fq2) {
        let rec1 = r1?;
        let rec2 = r2?;
        process_records(
            cfg,
            rec1,
            rec2,
            stats,
            aligner,
            overlap_buf,
            al_buf,
            &mut view_data,
        )?
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_records(
    cfg: &Config,
    rec1: FastQRecord,
    rec2: FastQRecord,
    stats: &mut Stats,
    aligner: &mut Aligner,
    ov_buf: &mut Vec<u8>,
    al_buf: &mut Vec<u8>,
    view_data: &mut Option<&mut ViewStore>,
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

    let reference = cfg.reference();
    let min_qual = cfg.min_qual();
    let skip_mb_del = cfg.ignore_multibase_deletions();
    let skip_mult_del = cfg.ignore_multiple_deletions();
    let skip_mult_mut = cfg.ignore_multiple_mutations();
    let skip_mult_mod = cfg.ignore_multiple_modifications();

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
    aligner.set_alignment_free_ends(0, 15, 15, 0);

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
    aligner.set_alignment_free_ends(0, 0, 0, 0);

    aligner
        .align(ov_buf, reference)
        .with_context(|| "Error when aligning to reference")?;

    let cigar = aligner.wfs_aligner().cigar();
    let mut patt_itr = ov_buf.iter();
    let mut text_itr = reference.iter();
    al_buf.clear();
    let mut start_del = None;
    let mut n_del = 0;
    let mut n_mut = 0;
    let mut mb_del = false;
    for op in cigar.operations() {
        match *op {
            b'M' | b'X' => {
                let r = text_itr.next().unwrap();
                let p = patt_itr.next().unwrap();
                if let Some(x) = start_del.take() {
                    stats.add_del(x, al_buf.len())
                }
                if *r != *p {
                    n_mut += 1
                }
                al_buf.push(p.to_ascii_uppercase());
            }
            b'I' => {
                let _ = text_itr.next();
                al_buf.push(b' ');
                if start_del.is_some() {
                    mb_del = true
                } else {
                    start_del = Some(al_buf.len());
                    n_del += 1;
                }
            }
            b'D' => {
                let _ = patt_itr.next();
                if let Some(e) = al_buf.last_mut() {
                    *e = e.to_ascii_lowercase()
                }
                if let Some(x) = start_del.take() {
                    stats.add_del(x, al_buf.len())
                }
            }
            _ => panic!("Unknown operation"),
        }
    }

    al_buf[0] = al_buf[0].to_ascii_uppercase();
    let ix = al_buf.len();
    al_buf[ix - 1] = al_buf[ix - 1].to_ascii_uppercase();
    if let Some(x) = start_del.take() {
        stats.add_del(x, al_buf.len())
    }

    stats.add_mut_and_del_counts(n_mut, n_del);
    let skip = (skip_mb_del && mb_del)
        || (skip_mult_mut && n_mut > 1)
        || (skip_mult_del && n_del > 1)
        || (skip_mult_mod && (n_mut + n_del) > 1);

    if !skip {
        stats.add_obs(al_buf.as_ref());
        if let Some(vs) = view_data.as_mut() {
            let mut v_itr = vs.next_view().iter_mut();
            for p in al_buf.iter() {
                if let Some(q) = v_itr.next() {
                    *q = *p
                } else {
                    break;
                }
            }
            for q in v_itr {
                *q = b' ';
            }
        }
    }
    stats.add_len(ov_buf.len() as u32);
    Ok(())
}
