use std::thread;

use crossbeam_channel::{Receiver, Sender};

use super::{
    Buffer,
    counts::Stats,
};
use crate::cli::Config;

mod process_thread;
mod process_buffer;
use process_thread::process_thread;

pub fn process_threads<'a> (
    cfg: &'a Config,
    rcv: Receiver<Buffer>,
    snd: Sender<Buffer>,
) -> anyhow::Result<Stats<'a>> {
    let nt = cfg.threads();
    let mut error = None;
    let mut stats = Stats::new(cfg.reference());
    
    thread::scope(|scope| {
        debug!("Setting up process thread(s)");

        let process_handles: Vec<_> = (0..nt)
            .map(|ix| {
                let recv_buf = rcv.clone();
                let send_buf = snd.clone();
                scope.spawn(move || process_thread(cfg, ix, recv_buf, send_buf))
            })
            .collect();

        debug!("Waiting for process thread(s) to finish");

        // Wait for process threads to finish
        for jh in process_handles {
            match jh.join().expect("Error joining process threads") {
                Err(e) => {
                    if error.is_none() {
                        error = Some(e)
                    }
                }
                Ok(s) => stats += s,
            }
        }
    });

    debug!("Process thread(s) finished");

    if let Some(e) = error { Err(e) } else { Ok(stats) }
}
