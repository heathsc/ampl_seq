use std::{borrow::Cow, fmt, ops::{Deref, DerefMut}};

use crossbeam_channel::Sender;

const VIEW_N_REC: usize = 1024;

#[repr(transparent)]
pub struct ViewRec {
    inner: [u8],
}

impl Deref for ViewRec {
    type Target = [u8];
    
    fn deref(&self) -> &Self::Target {
        self.to_bytes()
    }
}

impl DerefMut for ViewRec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.to_bytes_mut()
    }
}

impl ViewRec {
    pub fn from_ptr(s: &[u8]) -> &ViewRec {
        // SAFETY: Casting to ViewRec is safe because its internal
        // reprsentaion is a [u8] too (safe only inside std).
        // Dereferencing the obtained pointer is safe because it comes from a
        // reference. Making a reference is then safe because its lifetime
        // is bound by the lifetime of the given `bytes`.
        unsafe { &*(s as *const [u8] as *const ViewRec) }
    }
    
    pub fn from_ptr_mut(s: &mut [u8]) -> &mut ViewRec {
        // SAFETY: See above for [Self::from_ptr]
        unsafe { &mut *(s as *mut [u8] as *mut ViewRec) }
    }
    
    pub const fn to_bytes(&self) -> &[u8] {
        &self.inner
    }
    
    pub const fn to_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.inner
    }
    
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.to_bytes())
    }
}

impl fmt::Display for ViewRec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_lossy())
    }
}

impl fmt::Debug for ViewRec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub struct ViewBuf {
    inner: Box<[u8]>,
    rec_len: usize,
    n_rec: usize,
}

impl ViewBuf {
    pub fn new(rec_len: usize) -> Self {
        assert!(rec_len > 0, "Record lengths cannot be zero");
        let l = VIEW_N_REC.checked_mul(rec_len).expect("rec_len too large");
        let inner = vec![0; l].into_boxed_slice();
        Self {
            inner,
            rec_len,
            n_rec: 0,
        }
    }

    fn is_full(&self) -> bool {
        self.n_rec * self.rec_len >= self.inner.len()
    }

    fn next_mut(&mut self) -> Option<&mut ViewRec> {
        let i = self.n_rec;
        let l = self.rec_len;
        if (i + 1) * l <= self.inner.len() {
            self.n_rec += 1;
            Some(ViewRec::from_ptr_mut(&mut self.inner[i * l..(i + 1) * l]))
        } else {
            None
        }
    }
    
    pub fn recs(&self) -> impl Iterator<Item = &ViewRec> {
        self.inner.chunks_exact(self.rec_len).map(ViewRec::from_ptr)
    }
    
}

pub struct ViewStore {
    inner: Option<ViewBuf>,
    snd: Sender<ViewBuf>,
    rec_len: usize,
}

impl ViewStore {
    pub fn new(rec_len: usize, snd: Sender<ViewBuf>) -> Self {
        Self {
            inner: None,
            snd,
            rec_len,
        }
    }

    pub fn next_view(&mut self) -> &mut ViewRec {
        self.check_or_send();
        self.inner.as_mut().and_then(|v| v.next_mut()).unwrap()
    }

    pub fn flush(&mut self) {
        if let Some(v) = self.inner.take()
            && v.n_rec > 0
        {
            self.snd.send(v).expect("Error sending view for printing")
        }
    }

    fn check_or_send(&mut self) {
        if let Some(v) = self.inner.as_mut() {
            if !v.is_full() {
                return;
            }
            let v = self.inner.take().unwrap();
            self.snd.send(v).expect("Error sending view for printing")
        }
        self.inner = Some(ViewBuf::new(self.rec_len));
    }
}

impl Drop for ViewStore {
    fn drop(&mut self) {
        self.flush()
    }
}