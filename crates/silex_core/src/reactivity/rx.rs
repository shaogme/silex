use crate::{
    ErrorHandlerInput, OwnerAccess, ReadSignal, SilexError, SilexResult,
    reactivity::{Computed, StoredValue},
    traits::RxReadRef,
};
use silex_reactivity::{
    Computed as RxComputed, ReadSignal as RxReadSignal, StoredValue as RxStoredValue,
};

pub(crate) enum RxInner<'scope, T> {
    ReadSignal(RxReadSignal<'scope, T>),
    Computed(RxComputed<'scope, T, SilexError>),
    Stored(RxStoredValue<'scope, T>),
}

impl<'scope, T> Copy for RxInner<'scope, T> {}

impl<'scope, T> Clone for RxInner<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> PartialEq for RxInner<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ReadSignal(a), Self::ReadSignal(b)) => a == b,
            (Self::Computed(a), Self::Computed(b)) => a == b,
            (Self::Stored(a), Self::Stored(b)) => a == b,
            _ => false,
        }
    }
}

impl<'scope, T> Eq for RxInner<'scope, T> {}

/// A typed read-only reactive value tied to its creating owner.
pub struct Rx<'scope, T> {
    pub(crate) inner: RxInner<'scope, T>,
    pub(crate) owner: OwnerAccess<'scope>,
}

impl<'scope, T> Copy for Rx<'scope, T> {}

impl<'scope, T> Clone for Rx<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> PartialEq for Rx<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.owner == other.owner
    }
}

impl<'scope, T> Eq for Rx<'scope, T> {}

impl<'scope, T: 'scope> Rx<'scope, T> {
    pub fn into_rx(self) -> Self {
        self
    }

    pub(crate) fn from_signal(signal: ReadSignal<'scope, T>) -> Self {
        Self {
            inner: RxInner::ReadSignal(signal.inner),
            owner: signal.owner,
        }
    }

    pub(crate) fn from_computed(computed: Computed<'scope, T>) -> Self {
        Self {
            inner: RxInner::Computed(computed.inner),
            owner: computed.owner,
        }
    }

    pub(crate) fn from_stored(stored: StoredValue<'scope, T>) -> Self {
        Self {
            inner: RxInner::Stored(stored.inner),
            owner: stored.owner,
        }
    }

    pub fn owner(&self) -> OwnerAccess<'scope> {
        self.owner
    }

    pub fn map<U, F, H>(self, f: F, error_handler: H) -> SilexResult<Rx<'scope, U>>
    where
        U: 'scope,
        F: Fn(&T) -> U + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let owner = self.owner;
        owner
            .computed_always(move || self.with(|value| f(value)), error_handler)
            .map(Computed::into_rx)
    }

    pub fn is_constant(&self) -> bool {
        matches!(self.inner, RxInner::Stored(_))
    }
}
