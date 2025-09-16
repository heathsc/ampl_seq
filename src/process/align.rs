use rust_wfa2::{
    aligner::WfaAligner,
    alignment::{AlignmentScope, Attributes},
    error::WfaStatus,
};

pub struct Aligner {
    aligner: WfaAligner,
    buf: Vec<u8>,
}

impl Default for Aligner {
    fn default() -> Self {
        let mut attributes = Attributes::default();
        attributes.set_affine_penalties(0, 4, 6, 2);
        attributes.set_alignment_scope(AlignmentScope::Alignment);
        let aligner = WfaAligner::new(&attributes);
        Self {
            aligner,
            buf: Vec::new(),
        }
    }
}

impl Aligner {
    pub fn wfs_aligner(&self) -> &WfaAligner {
        &self.aligner
    }

    pub fn wfs_aligner_mut(&mut self) -> &mut WfaAligner {
        &mut self.aligner
    }

    pub fn buf_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }

    pub fn buf(&self) -> &[u8] {
        &self.buf
    }
    pub fn align_buf_as_text(&mut self, pattern: &[u8]) -> anyhow::Result<WfaStatus> {
        self.aligner
            .align(pattern, &self.buf)
            .map_err(|e| anyhow!(e))
    }
}
