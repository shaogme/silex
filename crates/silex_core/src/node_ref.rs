use std::{fmt, marker::PhantomData};

/// A scope-owned host object reference.
pub struct NodeRef<'scope, 'run, T = ()> {
    pub(crate) inner: silex_reactivity::NodeRef<'scope, 'run, T>,
}

impl<'scope, 'run, T> Copy for NodeRef<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for NodeRef<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for NodeRef<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRef").finish_non_exhaustive()
    }
}

impl<'scope, 'run, T: 'scope> NodeRef<'scope, 'run, T> {
    pub(crate) fn from_inner(inner: silex_reactivity::NodeRef<'scope, 'run, T>) -> Self {
        Self { inner }
    }

    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.inner.get()
    }

    pub fn load(&self, value: T) -> bool {
        self.inner.set(value).is_ok()
    }

    pub fn clear(&self) -> bool {
        self.inner.clear().is_ok()
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}

#[allow(dead_code)]
type _NodeRefMarker<T> = PhantomData<T>;
