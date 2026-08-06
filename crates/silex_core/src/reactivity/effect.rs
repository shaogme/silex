use std::{fmt, marker::PhantomData};

/// High-level effect handle.
pub struct Effect<'scope> {
    marker: PhantomData<fn(&'scope ()) -> &'scope ()>,
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

impl<'scope> Effect<'scope> {
    pub(crate) fn from_inner(inner: silex_reactivity::Effect<'scope>) -> Self {
        let _ = inner;
        Self {
            marker: PhantomData,
        }
    }
}
