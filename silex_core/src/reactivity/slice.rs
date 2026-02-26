use crate::traits::*;
use crate::traits::{RxCloneData, RxData};
use std::marker::PhantomData;

#[derive(Clone, Copy)]
pub struct SignalSlice<S, F, O: ?Sized> {
    source: S,
    getter: F,
    _marker: PhantomData<O>,
}

impl<S, F, O> SignalSlice<S, F, O>
where
    S: RxInternal + RxCloneData,
    F: Fn(&S::Value) -> &O + 'static,
    O: ?Sized + RxData,
{
    pub fn new(source: S, getter: F) -> Self {
        Self {
            source,
            getter,
            _marker: PhantomData,
        }
    }
}

/// 一个泛型投影守卫，通过持有源守卫来确保投影出的引用有效。
pub struct SliceGuard<G, O: ?Sized> {
    _source: G,
    value: *const O,
}

impl<G: std::ops::Deref, O: ?Sized> std::ops::Deref for SliceGuard<G, O> {
    type Target = O;
    #[inline(always)]
    fn deref(&self) -> &O {
        unsafe { &*self.value }
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
    S: RxRead + RxCloneData,
    F: Fn(&S::Value) -> &O + 'static,
    O: ?Sized + RxData,
{
    #[inline(always)]
    fn id(&self) -> Option<crate::reactivity::NodeId> {
        self.source.id()
    }

    #[inline(always)]
    fn track(&self) {
        self.source.track();
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.source.is_disposed()
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        self.source.defined_at()
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.source.debug_name().map(|n| format!("{}.slice", n))
    }
}

impl<S, F, O> RxInternal for SignalSlice<S, F, O>
where
    S: RxRead + RxCloneData,
    for<'a> S::ReadOutput<'a>: std::ops::Deref<Target = S::Value>,
    F: Fn(&S::Value) -> &O + 'static,
    O: ?Sized + RxData,
{
    type ReadOutput<'a>
        = SliceGuard<S::ReadOutput<'a>, O>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        let source_guard = self.source.rx_read_untracked()?;
        // 安全地获取指针：通过块作用域终止对 source_guard 的生命周期借用，
        // 然后将其移动到 SliceGuard 中。由于转换是在投影引用上进行的，这在 rx_read 函数体内是安全的。
        let value_ptr = {
            let val_ref = &*source_guard;
            (self.getter)(val_ref) as *const O
        };
        Some(SliceGuard {
            _source: source_guard,
            value: value_ptr,
        })
    }

    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.source
            .rx_try_with_untracked(|val| fun((self.getter)(val)))
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        self.source.rx_is_constant()
    }
}

impl<S, F, O> IntoRx for SignalSlice<S, F, O>
where
    S: RxRead + RxCloneData,
    F: Fn(&S::Value) -> &O + 'static,
    O: ?Sized + RxData,
{
    type RxType = crate::Rx<Self, crate::RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        crate::Rx(self, PhantomData)
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.source.rx_is_constant()
    }
}

impl<S, F, O> crate::traits::IntoSignal for SignalSlice<S, F, O>
where
    S: RxRead + RxCloneData,
    for<'a> S::ReadOutput<'a>: std::ops::Deref<Target = S::Value>,
    F: Fn(&S::Value) -> &O + 'static,
    O: ?Sized + Clone + RxData,
{
    #[inline(always)]
    fn into_signal(self) -> crate::reactivity::Signal<Self::Value>
    where
        Self: 'static,
        O: Sized,
    {
        crate::reactivity::Signal::derive(Box::new(move || self.get()))
    }
}

crate::impl_reactive_ops!(SignalSlice<S, F, O>, [S, F, O]);
