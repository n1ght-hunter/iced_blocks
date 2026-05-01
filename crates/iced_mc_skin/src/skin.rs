use std::sync::Arc;

/// Cheaply-cloneable handle to the raw 64×64 RGBA pixel data of a Minecraft
/// player skin.
#[derive(Clone)]
pub struct Skin(Arc<Vec<u8>>);

impl std::fmt::Debug for Skin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Skin").finish()
    }
}

impl Skin {
    /// Creates a new skin from raw RGBA pixel data.
    pub fn create(skin_rgba: Vec<u8>) -> Self {
        Self(Arc::new(skin_rgba))
    }

    pub(crate) fn raw(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq for Skin {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
