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

impl<S, F, O> RxInternal for SignalSlice<S, F, O>
where
    S: Read + Clone + 'static,
    for<'a> <S as RxInternal>::ReadOutput<'a>: std::ops::Deref<Target = S::Value>,
    F: Fn(&<S as RxInternal>::Value) -> &O + Clone + 'static,
    O: ?Sized + 'static,
{
    type Value = O;
    type ReadOutput<'a>
        = SliceGuard<S::ReadOutput<'a>, O>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_track(&self) {
        self.source.rx_track();
    }

    #[inline(always)]
    fn rx_read(&self) -> Option<Self::ReadOutput<'_>> {
        self.rx_track();
        self.rx_read_untracked()
    }

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

    fn rx_defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        self.source.rx_defined_at()
    }

    fn rx_debug_name(&self) -> Option<String> {
        self.source.rx_debug_name().map(|n| format!("{}.slice", n))
    }

    fn rx_is_disposed(&self) -> bool {
        self.source.rx_is_disposed()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        self.source.rx_is_constant()
    }
}

impl<S, F, O> WithUntracked for SignalSlice<S, F, O>
where
    S: Read + WithUntracked<Value = <S as RxInternal>::Value> + Clone + 'static,
    for<'a> <S as RxInternal>::ReadOutput<'a>: std::ops::Deref<Target = <S as RxInternal>::Value>,
    F: Fn(&<S as RxInternal>::Value) -> &O + Clone + 'static,
    O: ?Sized + 'static,
{
    type Value = O;
    #[inline(always)]
    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        crate::traits::RxInternal::rx_try_with_untracked(self, fun)
    }
}

impl<S, F, O> IntoRx for SignalSlice<S, F, O>
where
    S: Read + WithUntracked<Value = <S as RxInternal>::Value> + Clone + 'static,
    for<'a> <S as RxInternal>::ReadOutput<'a>: std::ops::Deref<Target = <S as RxInternal>::Value>,
    F: Fn(&<S as RxInternal>::Value) -> &O + Clone + 'static,
    O: ?Sized + 'static,
{
    type Value = O;
    type RxType = crate::Rx<Self, crate::RxValue>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        crate::Rx(self, PhantomData)
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.source.rx_is_constant()
    }
    #[inline(always)]
    fn into_signal(self) -> crate::reactivity::Signal<Self::Value>
    where
        Self: 'static,
        O: Sized + Clone, // Changed from Self::Value: Sized + Clone to O: Sized + Clone
    {
        crate::reactivity::Signal::derive(move || self.get())
    }
}

crate::impl_reactive_ops!(SignalSlice<S, F, O>, [S, F, O]);
