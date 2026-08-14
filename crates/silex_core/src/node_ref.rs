use std::fmt;

use crate::{SilexError, SilexResult};

/// A scope-owned host object reference.
pub struct NodeRef<'scope, T = ()> {
    pub(crate) inner: silex_reactivity::NodeRef<'scope, T>,
}

impl<'scope, T> Copy for NodeRef<'scope, T> {}

impl<'scope, T> Clone for NodeRef<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for NodeRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRef").finish_non_exhaustive()
    }
}

impl<'scope, T: 'scope> NodeRef<'scope, T> {
    pub(crate) fn from_inner(inner: silex_reactivity::NodeRef<'scope, T>) -> Self {
        Self { inner }
    }

    pub fn get(&self) -> SilexResult<Option<T>>
    where
        T: Clone,
    {
        self.inner.get().map_err(SilexError::fatal)
    }

    pub fn load(&self, value: T) -> SilexResult<()> {
        self.inner.set(value).map_err(SilexError::fatal)
    }

    pub fn clear(&self) -> SilexResult<()> {
        self.inner.clear().map_err(SilexError::fatal)
    }
}
