use std::{
    fs::File,
    io::{BufWriter, Write},
};

use anyhow::Context;

use crate::cli::Config;

#[derive(Debug, Default)]
pub struct Counts {
    // Counts for A, C, G, T
    base_counts: [u64; 4],
}

pub struct Stats {
    pos_counts: Vec<Counts>,
}

impl Stats {
    pub fn new(size: usize) -> Self {
        let pos_counts: Vec<_> = (0..size).map(|_| Counts::default()).collect();
        Self { pos_counts }
    }

    pub fn add_obs(&mut self, p: &[u8]) {
        for (c, cts) in p
            .iter()
            .zip(self.pos_counts.iter_mut().map(|c| &mut c.base_counts))
        {
            match *c {
                b'a' | b'A' => cts[0] += 1,
                b'c' | b'C' => cts[1] += 1,
                b'g' | b'G' => cts[2] += 1,
                b't' | b'T' => cts[3] += 1,
                _ => {}
            }
        }
    }

    pub fn output(&self, cfg: &Config) -> anyhow::Result<()> {
        let out_name = format!("{}_stats.tsv", cfg.output_prefix());
        let mut wrt = BufWriter::new(
            File::create(&out_name).with_context(|| "Could not open output file {out_name}")?,
        );

        let rf = cfg.reference();

        writeln!(
            wrt,
            "Pos\tRef\tN(A)\tN(C)\tN(G)\tN(T)\tTot\t%A\t%C\t%G\t%T\t%Miss"
        )?;
        
        for (ix, (ct, r)) in self.pos_counts.iter().map(|c| &c.base_counts).zip(rf.iter()).enumerate() {
            write!(wrt, "{ix}\t{}", *r as char)?;
            let n = ct.iter().sum::<u64>();
            write!(wrt, "\t{}\t{}\t{}\t{}\t{n}", ct[0], ct[1], ct[2], ct[3])?;
            let n = n as f64;
            let j = match *r {
                b'a' | b'A' => 0,
                b'c' | b'C' => 1,
                b'g' | b'G' => 2,
                b't' | b'T' => 3,
                _ => 4,
            };
            let mut miss = 0;
            for (i, x) in ct.iter().enumerate() {
                let z = *x as f64 * 100.0 / n;
                if i != j && j < 4 {
                    miss += *x
                }
                write!(wrt,"\t{z:.2}")?;
            }
            writeln!(wrt,"\t{:.2}", miss as f64 * 100.0 / n)?;
        }
        Ok(())
    }
}
