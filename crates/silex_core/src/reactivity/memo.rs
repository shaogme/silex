use crate::{ErrorHandlerInput, OwnerAccess, ReadGuard, Rx, SilexError, SilexResult};
use crate::{
    callback::map_callback_error,
    traits::{
        ReactiveInput, RuntimeScoped, RxBase, RxFrom, RxGet, RxRead, RxReadRef, RxReadRefSource,
        RxValue,
    },
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

    pub fn into_rx(self) -> Rx<'owner, T> {
        Rx::from_computed(self)
    }
}

impl<'owner, T: Clone + PartialEq + 'owner> RxFrom<'owner> for Computed<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        let value = value.into();
        let handler = owner.error_handler(|_: SilexError| {
            unreachable!("constant computed cannot report a user error")
        })?;
        owner.computed(move || Ok::<T, SilexError>(value.clone()), handler)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Computed<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Computed<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T> RuntimeScoped for Computed<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RxValue for Computed<'owner, T> {
    type Owned = T;
}

impl<'owner, T> RxBase for Computed<'owner, T> {
    fn track(&self) -> SilexResult<()> {
        self.inner.track().map_err(map_callback_error)
    }
}

impl<'owner, T> RxRead for Computed<'owner, T> {
    type ReadGuard<'a>
        = ReadGuard<'owner, T>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.inner
            .read()
            .map(ReadGuard::new)
            .map_err(map_callback_error)
    }

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.inner
            .read_untracked()
            .map(ReadGuard::new)
            .map_err(map_callback_error)
    }
}

impl<'owner, T> RxReadRefSource for Computed<'owner, T> {
    type ViewGuard<'a>
        = ReadGuard<'owner, T>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read()
    }

    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read_untracked()
    }
}

impl<'owner, T: Clone> RxGet for Computed<'owner, T> {
    fn get_untracked(&self) -> SilexResult<Self::Owned> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Owned> {
        self.with(Clone::clone)
    }
}
