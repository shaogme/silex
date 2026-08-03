use crate::{
    Rx, Scope, Signal,
    traits::{IntoRx, IntoSignal, RxBase, RxCloneData, RxData, RxRead, RxValue},
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

impl<S, F, O> RxBase for SignalSlice<S, F, O>
where
    S: RxBase,
    O: ?Sized + RxData,
{
    fn track(&self) {
        self.source.track();
    }

    fn is_alive(&self) -> bool {
        self.source.is_alive()
    }
}

impl<S, F, O> RxRead for SignalSlice<S, F, O>
where
    S: RxRead,
    F: Fn(&S::Value) -> &O,
    O: ?Sized + RxData,
{
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.source.try_with(|value| f((self.getter)(value)))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.source
            .try_with_untracked(|value| f((self.getter)(value)))
    }
}

impl<'scope, 'run, S, F, O> IntoRx<'scope, 'run> for SignalSlice<S, F, O>
where
    S: RxRead + 'scope,
    F: Fn(&S::Value) -> &O + 'scope,
    O: RxCloneData + 'scope,
{
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, O> {
        let scope = *scope;
        scope.derived(move || self.with(|value| value.clone()))
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<'scope, 'run, S, F, O> IntoSignal<'scope, 'run> for SignalSlice<S, F, O>
where
    S: RxRead + 'scope,
    F: Fn(&S::Value) -> &O + 'scope,
    O: RxCloneData + 'scope,
{
    fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, O> {
        self.into_rx(scope).into_signal(scope)
    }
}
