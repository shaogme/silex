use std::fmt;

/// High-level effect handle.
pub struct Effect<'scope, 'run> {
    pub(crate) inner: silex_reactivity::Effect<'scope, 'run>,
}

impl Copy for Effect<'_, '_> {}

impl Clone for Effect<'_, '_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for Effect<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Effect").finish_non_exhaustive()
    }
}

impl<'scope, 'run> Effect<'scope, 'run> {
    pub(crate) fn from_inner(inner: silex_reactivity::Effect<'scope, 'run>) -> Self {
        Self { inner }
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}
