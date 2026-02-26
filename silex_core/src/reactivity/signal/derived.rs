use crate::traits::*;
use crate::{Rx, RxValueKind};
use silex_reactivity::NodeId;
use std::panic::Location;

// --- Constant ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Constant<T>(pub T);

impl<T: RxData> RxValue for Constant<T> {
    type Value = T;
}

impl<T: RxData> RxBase for Constant<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        None
    }
    #[inline(always)]
    fn track(&self) {}
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        false
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        Some("Constant".to_string())
    }
}

impl<T: RxData> RxInternal for Constant<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        Some(RxGuard::Borrowed {
            value: &self.0,
            token: None,
        })
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        Some(fun(&self.0))
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        true
    }
}

impl<T: RxCloneData> IntoRx for Constant<T> {
    type RxType = Rx<T, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::new_constant(self.0)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
}

impl<T: RxCloneData> crate::traits::IntoSignal for Constant<T> {
    #[inline(always)]
    fn into_signal(self) -> super::Signal<T> {
        super::Signal::derive(Box::new(move || self.get()))
    }
}

// --- DerivedPayload ---

#[derive(Clone, Copy)]
pub struct DerivedPayload<Deps, F> {
    pub(crate) deps: Deps,
    pub(crate) func: F,
}

impl<D: std::fmt::Debug, F> std::fmt::Debug for DerivedPayload<D, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedPayload")
            .field("deps", &self.deps)
            .field("func", &"Fn(...)")
            .finish()
    }
}

impl<D, F> DerivedPayload<D, F> {
    pub const fn new(deps: D, func: F) -> Self {
        Self { deps, func }
    }
}

// Unary / Map implementation
impl<S, F, U> RxValue for DerivedPayload<S, F>
where
    S: RxValue,
    F: Fn(&S::Value) -> U + 'static,
{
    type Value = U;
}

impl<S, F, U> RxBase for DerivedPayload<S, F>
where
    S: RxBase + RxInternal,
    F: Fn(&S::Value) -> U + 'static,
    U: 'static,
{
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        self.deps.id()
    }
    #[inline(always)]
    fn track(&self) {
        self.deps.track();
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.deps.is_disposed()
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.deps.defined_at()
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.deps.debug_name()
    }
}

impl<S, F, U> RxInternal for DerivedPayload<S, F>
where
    S: RxInternal,
    F: Fn(&S::Value) -> U + 'static,
    U: 'static,
{
    type ReadOutput<'a>
        = RxGuard<'a, U, U>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.deps
            .rx_try_with_untracked(|v| (self.func)(v))
            .map(RxGuard::Owned)
    }

    #[inline(always)]
    fn rx_try_with_untracked<URet>(&self, fun: impl FnOnce(&Self::Value) -> URet) -> Option<URet> {
        self.deps.rx_try_with_untracked(|v| {
            let u_val = (self.func)(v);
            fun(&u_val)
        })
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    fn rx_is_constant(&self) -> bool {
        self.deps.rx_is_constant()
    }
}

impl<S, F, U> IntoRx for DerivedPayload<S, F>
where
    S: RxInternal + Clone + 'static,
    F: Fn(&S::Value) -> U + 'static,
    U: RxCloneData,
{
    type RxType = Rx<U, RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::derive(Box::new(move || {
            use crate::traits::RxGet;
            self.get()
        }))
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.deps.rx_is_constant()
    }
}

impl<S, F, U> crate::traits::IntoSignal for DerivedPayload<S, F>
where
    S: RxRead + Clone + 'static,
    for<'a> S::ReadOutput<'a>: std::ops::Deref<Target = S::Value>,
    F: Fn(&S::Value) -> U + 'static,
    U: RxCloneData,
{
    #[inline(always)]
    fn into_signal(self) -> super::Signal<Self::Value> {
        use crate::traits::RxGet;
        super::Signal::derive(Box::new(move || self.get()))
    }
}
