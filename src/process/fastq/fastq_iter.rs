use super::{fastq_record::FastQRecord, lines::Lines};

pub struct FastQIter<'a> {
    inner: &'a [u8],
}

impl<'a> FastQIter<'a> {
    pub fn make(s: &'a [u8]) -> Self {
        Self { inner: s }
    }
}
impl<'a> Iterator for FastQIter<'a> {
    type Item = anyhow::Result<FastQRecord<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.is_empty() {
            None
        } else {
            Some({
                let mut itr = Lines::make(self.inner);
                let raw_id = itr.next();
                let seq = itr.next();
                let id2 = itr.next();
                let qual = itr.next();
                if let (Some(raw_id), Some(seq), Some(id2), Some(qual)) = (raw_id, seq, id2, qual) {
                    if raw_id.is_empty()
                        || raw_id[0] != b'@'
                        || id2.is_empty()
                        || id2[0] != b'+'
                        || seq.is_empty()
                        || seq.len() != qual.len()
                    {
                        Err(anyhow!("Invalid FASTQ record"))
                    } else {
                        let id = if raw_id.len() == 1 { &[] } else { &raw_id[1..] };
                        self.inner = itr.inner();
                        Ok(FastQRecord::make(id, seq, qual))
                    }
                } else {
                    Err(anyhow!("Incomplete FASTQ record"))
                }
            })
        }
    }
}
