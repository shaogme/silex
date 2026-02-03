use crate::traits::*;
use std::marker::PhantomData;

#[derive(Clone, Copy)]
pub struct SignalSlice<S, F, O: ?Sized> {
    source: S,
    getter: F,
    _marker: PhantomData<O>,
}

impl<S, F, O> SignalSlice<S, F, O>
where
    S: WithUntracked + Clone + 'static,
    F: Fn(&S::Value) -> &O + Clone + 'static,
    O: ?Sized + 'static,
{
    pub fn new(source: S, getter: F) -> Self {
        Self {
            source,
            getter,
            _marker: PhantomData,
        }
    }
}

impl<S, F, O> DefinedAt for SignalSlice<S, F, O>
where
    S: DefinedAt + 'static,
    O: ?Sized + 'static,
{
    // Added 'static for consistency if needed, though DefinedAt doesn't strictly need it on S
    fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        self.source.defined_at()
    }

    fn debug_name(&self) -> Option<String> {
        self.source.debug_name().map(|n| format!("{}.slice", n))
    }
}

impl<S, F, O> WithUntracked for SignalSlice<S, F, O>
where
    S: WithUntracked + Clone + 'static,
    F: Fn(&S::Value) -> &O + Clone + 'static,
    O: ?Sized + 'static,
{
    type Value = O;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.source
            .try_with_untracked(|val| fun((self.getter)(val)))
    }
}

impl<S, F, O> Track for SignalSlice<S, F, O>
where
    S: Track,
    O: ?Sized,
{
    fn track(&self) {
        self.source.track();
    }
}

impl<S, F, O> GetUntracked for SignalSlice<S, F, O>
where
    S: WithUntracked + Clone + 'static,
    F: Fn(&S::Value) -> &O + Clone + 'static,
    O: Clone + 'static,
{
    type Value = O;
    fn try_get_untracked(&self) -> Option<O> {
        self.try_with_untracked(|v| v.clone())
    }
}

impl<S, F, O> IsDisposed for SignalSlice<S, F, O>
where
    S: IsDisposed,
    O: ?Sized,
{
    fn is_disposed(&self) -> bool {
        self.source.is_disposed()
    }
}

impl<S, F, O> Get for SignalSlice<S, F, O>
where
    S: WithUntracked + Track + Clone + 'static,
    F: Fn(&S::Value) -> &O + Clone + 'static,
    O: Clone + 'static,
{
    type Value = O;

    fn try_get(&self) -> Option<Self::Value> {
        self.try_with(Clone::clone)
    }
}
