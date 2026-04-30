use std::sync::Arc;

#[derive(Clone)]
pub struct Source(Arc<Vec<u8>>);

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Source").finish()
    }
}

impl Source {
    /// Creates a new skin source from raw RGBA pixel data.
    pub fn create(skin_rgba: Vec<u8>) -> Self {
        Self(Arc::new(skin_rgba))
    }

    pub(crate) fn raw(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
