use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    ops::AddAssign,
};

use anyhow::Context;

use crate::cli::Config;

#[derive(Debug, Default)]
pub struct Counts {
    // Counts for A, C, G, T
    base_counts: [u64; 4],
}

impl AddAssign for Counts {
    fn add_assign(&mut self, rhs: Self) {
        for ix in 0..4 {
            self.base_counts[ix] += rhs.base_counts[ix]
        }
    }
}

pub struct Stats<'a> {
    pos_counts: Vec<Counts>,
    insert_len: InsertLength,
    mut_corr: MutCorr<'a>,
}

impl<'a> AddAssign for Stats<'a> {
    fn add_assign(&mut self, mut rhs: Self) {
        assert_eq!(self.pos_counts.len(), rhs.pos_counts.len());

        for (c1, c2) in self.pos_counts.iter_mut().zip(rhs.pos_counts.drain(..)) {
            c1.add_assign(c2)
        }
        self.insert_len += rhs.insert_len;
        self.mut_corr += rhs.mut_corr
    }
}

impl<'a> Stats<'a> {
    pub fn new(rf: &'a [u8]) -> Self {
        let size = rf.len();
        let pos_counts: Vec<_> = (0..size).map(|_| Counts::default()).collect();
        let insert_len = InsertLength::default();
        let mut_corr = MutCorr::new(rf);
        Self {
            pos_counts,
            insert_len,
            mut_corr,
        }
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

        self.mut_corr.add_obs(p);
    }

    pub fn add_len(&mut self, len: u32) {
        self.insert_len.add_len(len)
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

        for (ix, (ct, r)) in self
            .pos_counts
            .iter()
            .map(|c| &c.base_counts)
            .zip(rf.iter())
            .enumerate()
        {
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
                write!(wrt, "\t{z:.2}")?;
            }
            writeln!(wrt, "\t{:.2}", miss as f64 * 100.0 / n)?;
        }
        self.insert_len.output(cfg)?;
        self.mut_corr.output(cfg)
    }
}

#[derive(Default)]
pub struct InsertLength {
    hash: BTreeMap<u32, u64>,
}

impl AddAssign for InsertLength {
    fn add_assign(&mut self, rhs: Self) {
        for (l, c) in rhs.hash.iter() {
            let e = self.hash.entry(*l).or_default();
            *e += *c
        }
    }
}

impl InsertLength {
    pub fn add_len(&mut self, x: u32) {
        let e = self.hash.entry(x).or_default();
        *e += 1
    }

    fn output(&self, cfg: &Config) -> anyhow::Result<()> {
        let n = self.hash.values().sum::<u64>();
        if n > 0 {
            let out_name = format!("{}_insert_len.tsv", cfg.output_prefix());
            let mut wrt = BufWriter::new(
                File::create(&out_name).with_context(|| "Could not open output file {out_name}")?,
            );
            writeln!(wrt, "Length\tCount\t%")?;
            let n = n as f64;
            for (len, ct) in self.hash.iter() {
                let z = *ct as f64 * 100.0 / n;
                writeln!(wrt, "{len}\t{ct}\t{z:.2}")?
            }
        }
        Ok(())
    }
}

pub struct MutCorr<'a> {
    cts: Vec<[u64; 4]>,
    rf: &'a [u8],
}

impl<'a> AddAssign for MutCorr<'a> {
    fn add_assign(&mut self, rhs: Self) {
        assert_eq!(self.rf, rhs.rf);
        for (ct1, ct2) in self.cts.iter_mut().zip(rhs.cts.iter()) {
            for ix in 0..4 {
                ct1[ix] += ct2[ix]
            }
        }
    }
}

impl<'a> MutCorr<'a> {
    fn new(rf: &'a [u8]) -> Self {
        let len = rf.len();
        assert!(len > 1);
        let sz = (len * (len - 1)) >> 1;
        Self {
            cts: vec![[0; 4]; sz],
            rf,
        }
    }

    fn add_obs(&mut self, s: &[u8]) {
        let tst = |a: &u8, r: &u8| {
            if a == r {
                Some(0)
            } else if *a == b'N' {
                None
            } else {
                Some(1)
            }
        };

        let l = self.rf.len();
        assert_eq!(s.len(), l);
        let mut ct = self.cts.iter_mut();
        for (i, x) in s[..l - 1]
            .iter()
            .zip(self.rf[..l - 1].iter())
            .map(|(a, r)| tst(a, r))
            .enumerate()
        {
            if let Some(x) = x.map(|z| z << 1) {
                for y in s[i + 1..]
                    .iter()
                    .zip(self.rf[i + 1..].iter())
                    .map(|(a, r)| tst(a, r))
                {
                    let cts = ct.next().unwrap();
                    if let Some(y) = y {
                        cts[x | y] += 1;
                    }
                }
            } else {
                let _ = ct.nth(l - i - 2);
            }
        }
        assert_eq!(ct.next(), None);
    }

    fn output(&self, cfg: &Config) -> anyhow::Result<()> {
        let out_name = format!("{}_mut_corr.tsv", cfg.output_prefix());
        let mut wrt = BufWriter::new(
            File::create(&out_name).with_context(|| "Could not open output file {out_name}")?,
        );
        write!(wrt, "pos")?;
        let l = self.rf.len();
        for i in 0..l {
            write!(wrt, "\t{i}")?;
        }
        writeln!(wrt)?;

        for i in 0..l {
            write!(wrt, "{i}")?;
            for j in 0..l {
                let z = if i == j {
                    1.0
                } else {
                    let k = if i > j {
                        ((i * (i - 1)) >> 1) + j
                    } else {
                        ((j * (j - 1)) >> 1) + i
                    };
                    let cts = &self.cts[k];
                    let r1 = (cts[0] + cts[1]) as f64;
                    let r2 = (cts[2] + cts[3]) as f64;
                    let n = r1 + r2;
                    let c1 = (cts[0] + cts[2]) as f64;
                    let c2 = (cts[1] + cts[3]) as f64;
                    (n * cts[3] as f64 - r2 * c2) / (r1 * r2 * c1 * c2).sqrt()
                };
                write!(wrt, "\t{z:6.4}")?;
            }
            writeln!(wrt)?;
        }
        Ok(())
    }
}
