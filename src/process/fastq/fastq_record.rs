#[derive(Copy, Clone)]
pub struct FastQRecord<'a> {
    id: &'a [u8],
    seq: &'a [u8],
    qual: &'a [u8],
}

impl<'a> FastQRecord<'a> {
    #[allow(dead_code)]
    #[inline]
    pub fn id(&self) -> &[u8] {
        self.id
    }

    #[inline]
    pub fn qual(&self) -> &[u8] {
        self.qual
    }

    #[inline]
    pub fn seq(&self) -> &[u8] {
        self.seq
    }

    pub(super) fn make(id: &'a [u8], seq: &'a [u8], qual: &'a [u8]) -> Self {
        Self { id, seq, qual }
    }
}
