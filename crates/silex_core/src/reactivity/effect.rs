use crate::{SilexError, SilexResult};
use std::fmt;

/// High-level effect handle.
pub struct EffectHandle<'scope> {
    pub(crate) inner: silex_reactivity::EffectHandle<'scope>,
}

impl Copy for EffectHandle<'_> {}

impl Clone for EffectHandle<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for EffectHandle<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectHandle").finish_non_exhaustive()
    }
}

impl<'scope> PartialEq for EffectHandle<'scope> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<'scope> Eq for EffectHandle<'scope> {}

impl<'scope> EffectHandle<'scope> {
    pub(crate) fn from_inner(inner: silex_reactivity::EffectHandle<'scope>) -> Self {
        Self { inner }
    }

    pub fn stop(&self) -> SilexResult<bool> {
        self.inner.stop().map_err(SilexError::fatal)
    }
}
