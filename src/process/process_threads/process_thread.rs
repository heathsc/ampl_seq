use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};

use super::process_buffer::process_buffer;

use crate::{
    cli::Config,
    process::{Buffer, align::Aligner, counts::Stats},
};

pub fn process_thread<'a> (
    cfg: &'a Config,
    ix: usize,
    rx: Receiver<Buffer>,
    sx: Sender<Buffer>,
) -> anyhow::Result<Stats<'a>> {
    debug!("Starting up process thread {ix}");

    let mut stats = Stats::new(cfg.reference());

    let mut overlap_align = Aligner::default();
    overlap_align
        .wfs_aligner_mut()
        .set_alignment_free_ends(0, 15, 15, 0);

    let mut overlap_buf = Vec::with_capacity(cfg.reference().len());
    while let Ok(mut b) = rx.recv() {
        if b.is_empty() {
            trace!(
                "Process thread {ix} received buffer {} for recycling",
                b.ix()
            )
        } else {
            trace!("Process thread {ix} received new buffer {}", b.ix());
            process_buffer(
                cfg,
                &b,
                &mut overlap_align,
                &mut stats,
                &mut overlap_buf
            )
            .with_context(|| format!("Process thread {ix}: Error parsing input buffer"))?;

            trace!(
                "Process thread {ix} finished processing block; sending empty block {} back to reader",
                b.ix()
            );

            b.clear();
        }
        // Send empty buffer to reader to be refilled.
        // Ignore errors when sending - this will happen when the reader process has finished
        let _ = sx.send(b);

    }
    debug!("Closing down process thread {ix}");

    Ok(stats)
}
