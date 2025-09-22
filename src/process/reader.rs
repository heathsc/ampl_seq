use std::{path::Path, thread};

use anyhow::Context;
use compress_io::compress::CompressIo;
use crossbeam_channel::{Receiver, Sender, bounded};

mod buffer;

pub use buffer::Buffer;

use crate::cli::Config;

fn get_buffer(recv: &Receiver<Buffer>) -> anyhow::Result<Buffer> {
    let mut b = recv
        .recv()
        .with_context(|| "Error receiving empty buffers")?;
    b.clear();
    Ok(b)
}

pub fn create_buffers(cfg: &Config, snd: &Sender<Buffer>) -> anyhow::Result<()> {
    let nb = cfg.threads().max(cfg.readers()) << 2;
    debug!("Number of buffers: {nb}");
    for ix in 0..nb {
        snd.send(Buffer::new(ix))?
    }
    Ok(())
}

fn read_thread(
    cfg: &Config,
    reader_ix: usize,
    rcv_buf: Receiver<Buffer>,
    snd_buf: Sender<Buffer>,
    recv_file: Receiver<usize>,
) -> anyhow::Result<()> {
    let mut pending: Option<Buffer> = None;

    debug!("Reader {reader_ix} starting up");

    while let Ok(ix) = recv_file.recv() {
        let f1 = &cfg.input_files()[ix << 1];
        let f2 = &cfg.input_files()[1 + (ix << 1)];
        read_from_fastq(reader_ix, f1, f2, &mut pending, &rcv_buf, &snd_buf)?;
        debug!("Reader {reader_ix}: Finished reading input file pair {ix}");
    }

    if let Some(buf) = pending.take() {
        assert!(buf.is_empty());
        snd_buf
            .send(buf)
            .with_context(|| "Erro sending buffer for recycling")?;
    }

    debug!("Reader {reader_ix} finished");
    Ok(())
}

fn read_from_fastq(
    reader_ix: usize,
    f1: &Path,
    f2: &Path,
    buf_store: &mut Option<Buffer>,
    rcv_buf: &Receiver<Buffer>,
    snd_buf: &Sender<Buffer>,
) -> anyhow::Result<()> {
    let open_file = |f| {
        CompressIo::new()
            .path(f)
            .reader()
            .with_context(|| "Could not open input file")
    };

    let mut rdr1 = open_file(f1)?;
    let mut rdr2 = open_file(f2)?;
    info!("Reader {reader_ix}: opened input files ({f1:?}, {f2:?})");

    let mut pending = match buf_store.take() {
        Some(b) => {
            trace!("Using existing buffer {}", b.ix());
            b
        }
        None => {
            trace!("Asking for new buffer");
            get_buffer(rcv_buf)?
        }
    };
    // Main loop - read files until empty
    loop {
        let mut b = pending;
        pending = get_buffer(rcv_buf)?;

        let eof = b.fill([&mut rdr1, &mut rdr2], &mut pending)?;
        trace!("Filled buffer: used {:?}", b.used());
        snd_buf
            .send(b)
            .with_context(|| "Error sending full buffer")?;
        if eof {
            if !pending.is_empty() {
                snd_buf
                    .send(pending)
                    .with_context(|| "Error sending full buffer")?;
            } else {
                *buf_store = Some(pending)
            }
            break;
        }
    }
    info!("Reader {reader_ix} Finished reading input files ({f1:?}, {f2:?})");
    Ok(())
}

pub fn reader(cfg: &Config, rcv: Receiver<Buffer>, snd: Sender<Buffer>) -> anyhow::Result<()> {
    let nr = cfg.readers();
    let mut error = None;

    thread::scope(|scope| {
        debug!("Setting up reader(s)");

        let (file_send, file_recv) = bounded(nr);

        let reader_handles: Vec<_> = (0..nr)
            .map(|ix| {
                let recv_buf = rcv.clone();
                let send_buf = snd.clone();
                let file_recv = file_recv.clone();

                scope.spawn(move || read_thread(cfg, ix, recv_buf, send_buf, file_recv))
            })
            .collect();

        drop(file_recv);

        // Send file indices to readers
        let nf = cfg.input_files().len() >> 1;
        for ix in 0..nf {
            file_send.send(ix).expect("Error sending file index to readers")
        }

        drop(file_send);

        debug!("Waiting for readers to finish");

        // Wait for readers to finish
        for jh in reader_handles {
            if let Err(e) = jh.join().expect("Error joining reader threads")
                && error.is_none()
            {
                error = Some(e)
            }
        }
    });

    debug!("Reader(s) finished");

    if let Some(e) = error { Err(e) } else { Ok(()) }
}
