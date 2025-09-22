use anyhow::Context;
use std::io::Read;

use crate::process::FastQIter;

const BUF_SIZE: usize = 1048576;

pub struct Buffer {
    inner: [Box<[u8]>; 2],
    used: [usize; 2],
    ix: usize,
}

impl Buffer {
    pub fn new(ix: usize) -> Self {
        let v1 = vec![0u8; BUF_SIZE].into_boxed_slice();
        let v2 = vec![0u8; BUF_SIZE].into_boxed_slice();
        let inner = [v1, v2];
        Self {
            inner,
            used: [0; 2],
            ix,
        }
    }

    pub fn ix(&self) -> usize {
        self.ix
    }

    pub fn used(&self) -> &[usize] {
        &self.used
    }

    /// Fills buffer unless EOF or an error occurs
    /// If buffer is full, returns a slice containing the possibly incomplete last entry,
    /// otherwise it returns an empty slice
    pub(super) fn fill<R: Read>(
        &mut self,
        rdr: [&mut R; 2],
        rem: &mut Self,
    ) -> anyhow::Result<bool> {
        // Fill both individual buffers
        let eof1 = self.fill_single_buf(rdr[0], 0)?;
        let eof2 = self.fill_single_buf(rdr[1], 1)?;
        
        // Step through records to find the last Ipossibly incomplete) record common to both buffers
        if let Some((ix1, ix2)) = self.inner[0][..self.used[0]]
            .split(|c| *c == b'\n')
            .zip(self.inner[1][..self.used[1]].split(|c| *c == b'\n'))
            .step_by(4)
            .last()
            .map(|(s1, s2)| {
                (
                    s1.as_ptr().addr() - self.inner[0].as_ptr().addr(),
                    s2.as_ptr().addr() - self.inner[1].as_ptr().addr(),
                )
            })
        {
            self.set_used_and_rem(ix1, 0, rem);
            self.set_used_and_rem(ix2, 1, rem);
            Ok(eof1 && eof2)
        } else {
            Err(anyhow!("Buffer to small for complete FASTQ record"))
        }
    }

    /// Fills buffer from rdr
    /// Returns true at EOF
    fn fill_single_buf<R: Read>(&mut self, rdr: &mut R, ix: usize) -> anyhow::Result<bool> {
        let b = &mut self.inner[ix];
        loop {
            let l = rdr
                .read(&mut b[self.used[ix]..])
                .with_context(|| "Error reading from input")?;

            self.used[ix] += l;

            // If buffer is full or we are at EOF
            if self.used[ix] == b.len() || l == 0 {
                break;
            }
        }
        Ok(self.used[ix] < b.len())
    }

    fn set_used_and_rem(&mut self, ix: usize, i: usize, rem: &mut Self) {
        let r = &self.inner[i][ix..self.used[i]];
        let l = r.len();
        self.used[i] = ix;
        rem.inner[i][..l].copy_from_slice(r);
        rem.used[i] = l
    }

    #[inline]
    pub fn as_slices(&self) -> (&[u8], &[u8]) {
        (
            &self.inner[0][..self.used[0]],
            &self.inner[1][..self.used[1]],
        )
    }

    #[inline]
    pub fn fastq<'a>(&'a self) -> (FastQIter<'a>, FastQIter<'a>) {
        let (s1, s2) = self.as_slices();

        (FastQIter::make(s1), FastQIter::make(s2))
    }

    #[inline]
    pub fn clear(&mut self) {
        self.used[0] = 0;
        self.used[1] = 0;
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.used[0] + self.used[1] == 0
    }
}
