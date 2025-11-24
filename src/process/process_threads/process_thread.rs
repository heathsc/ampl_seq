use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};

use super::{super::view::{ViewBuf, ViewStore}, process_buffer::process_buffer};

use crate::{
    cli::Config,
    process::{Buffer, align::Aligner, counts::Stats},
};

pub fn process_thread<'a>(
    cfg: &'a Config,
    ix: usize,
    rx: Receiver<Buffer>,
    sx: Sender<Buffer>,
    mut sx_view: Option<Sender<ViewBuf>>,
) -> anyhow::Result<Stats<'a>> {
    debug!("Starting up process thread {ix}");

    let mut stats = Stats::new(cfg.reference());

    let mut aligner = Aligner::default();

    let mut view_data = sx_view.take().map(|s| {
       ViewStore::new(cfg.reference().len(), s) 
    });

    let mut overlap_buf = Vec::with_capacity(cfg.reference().len());
    let mut al_buf = Vec::with_capacity(cfg.reference().len());

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
                &mut aligner,
                &mut stats,
                &mut overlap_buf,
                &mut al_buf,
                view_data.as_mut(),
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
