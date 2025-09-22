pub struct Lines<'a> {
    inner: &'a [u8],
}

impl <'a> Lines<'a> {
    pub(super) fn make(s: &'a [u8]) -> Self {
        Self { inner: s }
    }
    
    pub(super) fn inner(&self) -> &'a [u8] {
        self.inner
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.is_empty() {
            None
        } else if let Some((i, _)) = self.inner.iter().enumerate().find(|(_, c)| **c == b'\n') {
            let (s1, s2) = self.inner.split_at(i);
            self.inner = &s2[1..];
            Some(s1)
        } else {
            let s = self.inner;
            self.inner = &[];
            Some(s)
        }
    }
}

