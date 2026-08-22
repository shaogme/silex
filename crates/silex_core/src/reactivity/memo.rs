use crate::{
    ErrorHandlerInput, OwnerAccess, Rx, RxValueKind, SilexError, SilexResult, traits::RxRead,
};
use crate::{
    callback::map_callback_error,
    traits::{RuntimeScoped, RxBase, RxValue},
};
use std::fmt;

/// Unified computed value for equality-gated and always-notifying computations.
pub struct Computed<'owner, T> {
    pub(crate) inner: silex_reactivity::Computed<'owner, T, SilexError>,
    pub(crate) owner: OwnerAccess<'owner>,
}

impl<'owner, T> Copy for Computed<'owner, T> {}

impl<'owner, T> Clone for Computed<'owner, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for Computed<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Computed").finish_non_exhaustive()
    }
}

impl<'owner, T> PartialEq for Computed<'owner, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.owner == other.owner
    }
}

impl<'owner, T> Eq for Computed<'owner, T> {}

impl<'owner, T: 'owner> Computed<'owner, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::Computed<'owner, T, SilexError>,
        owner: OwnerAccess<'owner>,
    ) -> Self {
        Self { inner, owner }
    }

    pub fn map<U, F, H>(self, f: F, error_handler: H) -> SilexResult<Rx<'owner, U>>
    where
        U: 'owner,
        F: Fn(&T) -> U + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let owner = self.owner;
        owner
            .computed_always(move || self.with(|value| f(value)), error_handler)
            .map(Computed::into_rx)
    }

    pub fn into_rx(self) -> Rx<'owner, T, RxValueKind> {
        Rx::from_computed(self)
    }
}

impl<'owner, T> RuntimeScoped for Computed<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RxValue for Computed<'owner, T> {
    type Value = T;
}

impl<'owner, T> RxBase for Computed<'owner, T> {
    fn track(&self) -> SilexResult<()> {
        self.inner.track().map_err(map_callback_error)
    }
}

impl<'owner, T> RxRead for Computed<'owner, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(map_callback_error)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(map_callback_error)
    }
}
