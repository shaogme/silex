use crate::{SilexError, SilexResult};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// A runtime-backed read guard with `silex_core` error conversion.
pub struct ReadGuard<'scope, T: ?Sized> {
    inner: silex_reactivity::ReadGuard<'scope, T>,
}

impl<'scope, T: ?Sized> ReadGuard<'scope, T> {
    pub(crate) fn new(inner: silex_reactivity::ReadGuard<'scope, T>) -> Self {
        Self { inner }
    }

    pub fn finish(self) -> SilexResult<()> {
        self.inner.finish().map_err(SilexError::fatal)
    }
}

impl<T: ?Sized> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A runtime-backed write guard with `silex_core` error conversion.
pub struct WriteGuard<'scope, T: ?Sized> {
    inner: silex_reactivity::WriteGuard<'scope, T>,
}

impl<'scope, T: ?Sized> WriteGuard<'scope, T> {
    pub(crate) fn new(inner: silex_reactivity::WriteGuard<'scope, T>) -> Self {
        Self { inner }
    }

    pub fn commit(self) -> SilexResult<()> {
        self.inner.commit().map_err(SilexError::fatal)
    }

    pub fn abort(self) -> SilexResult<()> {
        self.inner.abort().map_err(SilexError::fatal)
    }
}

impl<T: ?Sized> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A read guard backed by a borrow of a non-runtime value.
pub struct BorrowedReadGuard<'a, T: ?Sized> {
    value: &'a T,
}

impl<'a, T: ?Sized> BorrowedReadGuard<'a, T> {
    pub(crate) fn new(value: &'a T) -> Self {
        Self { value }
    }
}

impl<T: ?Sized> Deref for BorrowedReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

/// A read guard containing an owned snapshot rather than a live payload borrow.
pub struct OwnedReadGuard<T> {
    value: T,
}

impl<T> OwnedReadGuard<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> Deref for OwnedReadGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// A read guard that retains its source guard while exposing a safe projection.
pub struct MappedReadGuard<G, F, O: ?Sized> {
    source: G,
    getter: F,
    marker: PhantomData<fn() -> O>,
}

impl<G, F, O: ?Sized> MappedReadGuard<G, F, O> {
    pub(crate) fn new(source: G, getter: F) -> Self {
        Self {
            source,
            getter,
            marker: PhantomData,
        }
    }
}

impl<G, F, O: ?Sized> Deref for MappedReadGuard<G, F, O>
where
    G: Deref,
    F: for<'a> Fn(&'a G::Target) -> &'a O,
{
    type Target = O;

    fn deref(&self) -> &Self::Target {
        (self.getter)(&self.source)
    }
}

/// A guard used by [`crate::Rx`] to erase its concrete read-source variant.
pub enum RxReadGuard<'scope, T> {
    ReadSignal(ReadGuard<'scope, T>),
    Computed(ReadGuard<'scope, T>),
    Stored(ReadGuard<'scope, T>),
}

impl<T> RxReadGuard<'_, T> {
    pub fn finish(self) -> SilexResult<()> {
        match self {
            Self::ReadSignal(guard) | Self::Computed(guard) | Self::Stored(guard) => guard.finish(),
        }
    }
}

impl<T> Deref for RxReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::ReadSignal(guard) | Self::Computed(guard) | Self::Stored(guard) => guard,
        }
    }
}
