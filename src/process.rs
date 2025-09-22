mod align;
mod counts;
mod fastq;
mod process_threads;
mod reader;

use crossbeam_channel::unbounded;

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

        reader::create_buffers(cfg, &empty_send).expect("Error creating buffers");

        let rx = full_recv.clone();
        let tx = empty_send.clone();
        let process_handle = scope.spawn(|| process_threads(cfg, rx, tx));

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
    });

    if let Some(e) = error {
        Err(e)
    } else if let Some(s) = stats.take() {
        s.output(cfg)
    } else {
        Err(anyhow!("No statistics were collected"))
    }
}
