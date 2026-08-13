use crate::{
    SilexResult,
    traits::{RxData, RxRead, RxValue},
};
use std::marker::PhantomData;

/// A safe projection over a source reactive value.
pub struct SignalSlice<S, F, O: ?Sized> {
    pub(crate) source: S,
    pub(crate) getter: F,
    marker: PhantomData<fn() -> O>,
}

impl<S, F, O: ?Sized> SignalSlice<S, F, O> {
    pub(crate) fn new(source: S, getter: F) -> Self {
        Self {
            source,
            getter,
            marker: PhantomData,
        }
    }
}

impl<S, F, O> RxValue for SignalSlice<S, F, O>
where
    O: ?Sized + RxData,
{
    type Value = O;
}

impl<S, F, O> RxRead for SignalSlice<S, F, O>
where
    S: RxRead,
    F: Fn(&S::Value) -> &O,
    O: ?Sized + RxData,
{
    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
        self.source.with(|value| f((self.getter)(value)))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
        self.source.with_untracked(|value| f((self.getter)(value)))
    }
}
