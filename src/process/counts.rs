use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufWriter, Write},
    ops::AddAssign,
};

use anyhow::Context;

use crate::cli::Config;

const COUNTS_N: usize = 6;

#[derive(Debug, Default)]
pub struct Counts {
    // Counts for A, C, G, T, Del, Ins
    base_counts: [u64; COUNTS_N],
}

impl AddAssign for Counts {
    fn add_assign(&mut self, rhs: Self) {
        for ix in 0..COUNTS_N {
            self.base_counts[ix] += rhs.base_counts[ix]
        }
    }
}

pub struct Stats<'a> {
    pos_counts: Vec<Counts>,
    insert_len: InsertLength,
    mut_corr: MutCorr<'a>,
    del_hash: HashMap<(usize, usize), usize>,
    n_reads: usize,
}

impl<'a> AddAssign for Stats<'a> {
    fn add_assign(&mut self, mut rhs: Self) {
        assert_eq!(self.pos_counts.len(), rhs.pos_counts.len());

        for (c1, c2) in self.pos_counts.iter_mut().zip(rhs.pos_counts.drain(..)) {
            c1.add_assign(c2)
        }
        self.insert_len += rhs.insert_len;
        self.mut_corr += rhs.mut_corr;

        for (k, v) in rhs.del_hash.iter() {
            *self.del_hash.entry(*k).or_default() += *v
        }

        self.n_reads += rhs.n_reads;
    }
}

impl<'a> Stats<'a> {
    pub fn new(rf: &'a [u8]) -> Self {
        let size = rf.len();
        let pos_counts: Vec<_> = (0..size).map(|_| Counts::default()).collect();
        let insert_len = InsertLength::default();
        let mut_corr = MutCorr::new(rf);
        let del_hash = HashMap::new();
        Self {
            pos_counts,
            insert_len,
            mut_corr,
            del_hash,
            n_reads: 0,
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
                b' ' => cts[4] += 1,
                _ => {}
            }
            if c.is_ascii_lowercase() {
                cts[5] += 1
            }
        }

        self.n_reads += 1;
        self.mut_corr.add_obs(p);
    }

    #[inline]
    pub fn add_del(&mut self, a: usize, b: usize) {
        *self.del_hash.entry((a, b)).or_default() += 1
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
            "Pos\tRef\tN(A)\tN(C)\tN(G)\tN(Del)\tN(Ins)\tN(T)\tTot\t%A\t%C\t%G\t%T\t%Del\t%Ins\t%Mut"
        )?;

        for (ix, (ct, r)) in self
            .pos_counts
            .iter()
            .map(|c| &c.base_counts)
            .zip(rf.iter())
            .enumerate()
        {
            write!(wrt, "{}\t{}", ix + 1, *r as char)?;
            let n = ct[..COUNTS_N - 1].iter().sum::<u64>();
            write!(
                wrt,
                "\t{}\t{}\t{}\t{}\t{}\t{}\t{n}",
                ct[0], ct[1], ct[2], ct[3], ct[4], ct[5]
            )?;
            let n = n as f64;
            let j = match *r {
                b'a' | b'A' => 0,
                b'c' | b'C' => 1,
                b'g' | b'G' => 2,
                b't' | b'T' => 3,
                _ => 4,
            };
            let mut mm = 0;
            for (i, x) in ct.iter().enumerate() {
                let z = *x as f64 * 100.0 / n;
                if i != j && j < 4 && i < 4 {
                    mm += *x
                }
                write!(wrt, "\t{z:.2}")?;
            }
            writeln!(wrt, "\t{:.2}", mm as f64 * 100.0 / n)?;
        }
        self.insert_len.output(cfg)?;
        let cm1 = self.mut_corr.output(cfg)?;

        self.output_del(cfg)?;
        let cm = self.mk_del_cm(cfg.reference().len());
        self.output_cm(cfg, &cm, &cm1)
    }

    fn mk_del_cm(&self, ref_len: usize) -> Vec<usize> {
        let mut cm = vec![0; ref_len * ref_len];
        for ((x, y), z) in self.del_hash.iter() {
            let x = *x - 1;
            let y = *y - 1;
            cm[x * ref_len + y] += *z;
            if x != y {
                cm[y * ref_len + x] += *z
            }
        }
        cm
    }

    fn output_cm(&self, cfg: &Config, cm: &[usize], cm1: &[[f64; 2]]) -> anyhow::Result<()> {
        let out_name = format!("{}_contact_map.tsv", cfg.output_prefix());
        let mut wrt = BufWriter::new(
            File::create(&out_name).with_context(|| "Could not open output file {out_name}")?,
        );
        writeln!(wrt, "x\ty\tdel%\tmm%\tr")?;
        let tot = self.n_reads as f64;
        let l = cfg.reference().len();
        for x in 0..l {
            for y in 0..l {
                let z = &cm1[x * l + y];
                writeln!(wrt, "{}\t{}\t{:8.5}\t{:8.5}\t{:7.5}", x + 1, y + 1, 100.0 * cm[x * l + y] as f64 / tot, 100.0 * z[0], z[1])?
            }
            writeln!(wrt)?
        }
        Ok(())
    }
    
    fn output_del(&self, cfg: &Config) -> anyhow::Result<()> {
        let mut v: Vec<_> = self.del_hash.iter().collect();
        v.sort_unstable_by(|((a1, b1), x1), ((a2, b2), x2)| match x2.cmp(x1) {
            Ordering::Equal => (b1 - a1).cmp(&(b2 - a2)),
            c => c,
        });

        let tot = self.n_reads as f64;

        let out_name = format!("{}_del.tsv", cfg.output_prefix());
        let mut wrt = BufWriter::new(
            File::create(&out_name).with_context(|| "Could not open output file {out_name}")?,
        );

        writeln!(wrt, "Start\tStop\tLen\tCount\t%")?;
        for ((a, b), x) in v.drain(..) {
            writeln!(
                wrt,
                "{}\t{}\t{}\t{}\t{:.2}",
                a,
                b,
                b + 1 - a,
                x,
                (100.0 * *x as f64) / tot
            )?
        }

        Ok(())
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
        let sz = (len * (len + 1)) >> 1;
        Self {
            cts: vec![[0; 4]; sz],
            rf,
        }
    }

    fn add_obs(&mut self, s: &[u8]) {
        let tst = |a: &u8, r: &u8| {
            if a == r {
                Some(0)
            } else if b"ACGTacgt".contains(a) {
                Some(1)
            } else {
                None
            }
        };

        let l = self.rf.len();
        assert_eq!(s.len(), l);
        let mut ct = self.cts.iter_mut();
        for (i, x) in s
            .iter()
            .zip(self.rf.iter())
            .map(|(a, r)| tst(a, r))
            .enumerate()
        {
            if let Some(x) = x.map(|z| z << 1) {
                for y in s[i..]
                    .iter()
                    .zip(self.rf[i..].iter())
                    .map(|(b, r)| tst(b, r))
                {
                    let cts = ct.next().unwrap();
                    if let Some(y) = y {
                        cts[x | y] += 1;
                    }
                }
            } else {
                let _ = ct.nth(l - i - 1);
            }
        }
        assert_eq!(ct.next(), None);
    }

    fn output(&self, cfg: &Config) -> anyhow::Result<Vec<[f64; 2]>> {
        let out_name = format!("{}_mut_corr.tsv", cfg.output_prefix());
        let mut wrt = BufWriter::new(
            File::create(&out_name).with_context(|| "Could not open output file {out_name}")?,
        );
        write!(wrt, "pos")?;
        let l = self.rf.len();
        
        let mut cm = vec!([0.0; 2]; l * l);
        for i in 1..=l {
            write!(wrt, "\t{i}")?;
        }
        writeln!(wrt)?;

        let get_k = |i, j| {
            if i > j {
                ((i * (i + 1)) >> 1) + j
            } else {
                ((j * (j + 1)) >> 1) + i
            }
        };
        
        for i in 0..l {
            write!(wrt, "{}", i + 1)?;
            for j in 0..l {
                let cts: &[u64; 4] = &self.cts[get_k(i, j)];
                let n = (cts[0] + cts[1] + cts[2] + cts[3]) as f64;
                let z = if i == j {
                    cm[i * l + i] = [cts[3] as f64 / n, 1.0];
                    1.0
                } else {
                    let r1 = (cts[0] + cts[1]) as f64;
                    let r2 = (cts[2] + cts[3]) as f64;
                    let n = r1 + r2;
                    let c1 = (cts[0] + cts[2]) as f64;
                    let c2 = (cts[1] + cts[3]) as f64;
                    let z = cts[3] as f64 / n;

                    let r = (n * cts[3] as f64 - r2 * c2) / (r1 * r2 * c1 * c2).sqrt();
                    cm[i * l + j] = [z, r];
                    cm[j * l + i] = [z, r];                    
                    r2
                };
                write!(wrt, "\t{z:6.4}")?;
            }
            writeln!(wrt)?;
        }
        Ok(cm)
    }
}
