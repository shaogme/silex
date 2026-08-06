use crate::{SilexError, SilexResult};
use std::fmt;

/// High-level effect handle.
pub struct Effect<'scope> {
    pub(crate) inner: silex_reactivity::Effect<'scope>,
}

impl Copy for Effect<'_> {}

impl Clone for Effect<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for Effect<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Effect").finish_non_exhaustive()
    }
}

impl<'scope> PartialEq for Effect<'scope> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<'scope> Eq for Effect<'scope> {}

impl<'scope> Effect<'scope> {
    pub(crate) fn from_inner(inner: silex_reactivity::Effect<'scope>) -> Self {
        Self { inner }
    }

    pub fn try_stop(&self) -> SilexResult<bool> {
        self.inner.try_stop().map_err(SilexError::from)
    }

    pub fn stop(&self) {
        self.try_stop()
            .unwrap_or_else(|error| panic!("停止 effect 失败: {error}"));
    }
}
