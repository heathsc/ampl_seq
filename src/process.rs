mod align;
mod counts;
mod fastq;
mod process_threads;
mod reader;
mod view;
mod write_view;

use crossbeam_channel::{bounded, unbounded};

pub use fastq::FastQIter;
use process_threads::process_threads;

pub use reader::Buffer;

use std::thread;

use super::cli::Config;

pub fn process(cfg: &Config) -> anyhow::Result<()> {
    let mut stats = None;
    let mut error = None;
 
    thread::scope(|scope| {
        // Channel used to send full buffers to process threads
        let (full_send, full_recv) = unbounded();

        // Channel used to send and receive empty buffers
        let (empty_send, empty_recv) = unbounded();

        // CHannel for vuew records
        let mut view_chan = if cfg.view_file() {
            Some(bounded(cfg.threads() * 2))
        } else {
            None
        };

        let mut view_writer_handle = view_chan.as_ref().map(|(_, r)| {
            let rx = r.clone();
            scope.spawn(|| write_view::write_view(cfg, rx))
        }); 
        
        reader::create_buffers(cfg, &empty_send).expect("Error creating buffers");

        let rx = full_recv.clone();
        let tx = empty_send.clone();
        let tx_view = view_chan.as_ref().map(|(t,_)| {
            t.clone()
        });
        let process_handle = scope.spawn(|| process_threads(cfg, rx, tx, tx_view));

        drop(full_recv);
        drop(empty_send);

        if let Err(e) = reader::reader(cfg, empty_recv, full_send) {
            error = Some(anyhow!(e))
        }

        match process_handle.join().expect("Error joining process thread") {
            Ok(s) => stats = Some(s),
            Err(e) => {
                if error.is_none() {
                    error = Some(anyhow!(e))
                }
            }
        }
        
        view_chan.take();
        
        if let Some(h) = view_writer_handle.take() {
            let _ = h.join().expect("Error joining view writer thread");
        }
    });

    if let Some(e) = error {
        Err(e)
    } else if let Some(s) = stats.take() {
        s.output(cfg)
    } else {
        Err(anyhow!("No statistics were collected"))
    }
}
