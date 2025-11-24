use std::io::Write;

use crossbeam_channel::Receiver;
use compress_io::compress::CompressIo;

use crate::cli::Config;

use super::view::ViewBuf;

pub fn write_view(cfg: &Config, rcv: Receiver<ViewBuf>) -> anyhow::Result<()> {
    
    debug!("Starting up view writer thread");
    
    let out_name = format!("{}_view.txt.gz", cfg.output_prefix());
    let mut wrt = CompressIo::new().path(&out_name).bufwriter()?;
    
    while let Ok(vb) = rcv.recv() {
        for r in vb.recs() {
            writeln!(wrt, "{}", r)?
        }
    }
        
    debug!("Closing down view writer thread");
    Ok(())
}