use crate::{
    Rx, RxValueKind, Scope,
    reactivity::{Memo, SignalSlice, StoredValue},
    traits::{IntoRx, IntoSignal, RxBase, RxRead, RxValue},
};
use silex_reactivity::{
    ReactiveResult, ReadSignal as RawReadSignal, WriteSignal as RawWriteSignal,
};
use std::{fmt, marker::PhantomData};

/// A plain value that has not yet been promoted into a runtime node.
#[derive(Clone, Debug, PartialEq)]
pub struct Constant<T>(pub(crate) T);

impl<T> Constant<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    pub const fn value(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> RxValue for Constant<T> {
    type Value = T;
}

impl<T> RxBase for Constant<T> {
    fn track(&self) {}
}

impl<T> RxRead for Constant<T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(f(&self.0))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(f(&self.0))
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for Constant<T> {
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T> {
        scope.constant(self.0)
    }

    fn is_constant(&self) -> bool {
        true
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for Constant<T> {
    fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T> {
        self.into_rx(scope).into_signal(scope)
    }
}

/// High-level read capability for a signal node.
pub struct ReadSignal<'scope, 'run, T> {
    pub(crate) inner: RawReadSignal<'scope, 'run, T>,
    pub(crate) scope: Scope<'scope, 'run>,
}

/// High-level write capability for a signal node.
pub struct WriteSignal<'scope, 'run, T> {
    pub(crate) inner: RawWriteSignal<'scope, 'run, T>,
}

/// A paired read/write signal.
pub struct RwSignal<'scope, 'run, T> {
    pub(crate) read: ReadSignal<'scope, 'run, T>,
    pub(crate) write: WriteSignal<'scope, 'run, T>,
}

/// A read-only union of the typed high-level node wrappers.
pub struct Signal<'scope, 'run, T> {
    pub(crate) rx: Rx<'scope, 'run, T, RxValueKind>,
}

impl<'scope, 'run, T> Copy for ReadSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for ReadSignal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for WriteSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for WriteSignal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for RwSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for RwSignal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for Signal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Signal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for ReadSignal<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadSignal").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for WriteSignal<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteSignal").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for RwSignal<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwSignal").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for Signal<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signal").finish_non_exhaustive()
    }
}

impl<'scope, 'run, T: 'scope> ReadSignal<'scope, 'run, T> {
    pub(crate) fn from_inner(
        inner: RawReadSignal<'scope, 'run, T>,
        scope: Scope<'scope, 'run>,
    ) -> Self {
        Self { inner, scope }
    }

    pub fn with_name(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.inner.try_get()
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner.get()
    }

    pub fn try_get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.inner.try_get_untracked()
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.inner.with(f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.inner
            .with_untracked(f)
            .expect("读取 scoped signal 失败")
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    pub fn into_rx(self) -> Rx<'scope, 'run, T> {
        Rx::from_signal(self)
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        O: ?Sized + 'scope,
        F: Fn(&T) -> &O + 'scope,
    {
        SignalSlice::new(self, getter)
    }
}

impl<'scope, 'run, T: 'scope> WriteSignal<'scope, 'run, T> {
    pub(crate) fn from_inner(inner: RawWriteSignal<'scope, 'run, T>) -> Self {
        Self { inner }
    }

    pub fn with_name(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn try_set(&self, value: T) -> ReactiveResult<()> {
        self.inner.try_set(value)
    }

    pub fn set(&self, value: T) {
        self.inner.set(value)
    }

    pub fn try_update<U>(&self, f: impl FnOnce(&mut T) -> U) -> ReactiveResult<U> {
        self.inner.try_update(f)
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.inner.update(f)
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}

impl<'scope, 'run, T> RwSignal<'scope, 'run, T> {
    pub(crate) fn from_parts(
        read: ReadSignal<'scope, 'run, T>,
        write: WriteSignal<'scope, 'run, T>,
    ) -> Self {
        Self { read, write }
    }

    pub fn read_signal(&self) -> ReadSignal<'scope, 'run, T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<'scope, 'run, T> {
        self.write
    }

    pub fn into_rx(self) -> Rx<'scope, 'run, T>
    where
        T: 'scope,
    {
        self.read.into_rx()
    }

    pub fn split(&self) -> (ReadSignal<'scope, 'run, T>, WriteSignal<'scope, 'run, T>) {
        (self.read, self.write)
    }

    pub fn get(&self) -> T
    where
        T: Clone + 'scope,
    {
        self.read.get()
    }

    pub fn set(&self, value: T)
    where
        T: 'scope,
    {
        self.write.set(value)
    }

    pub fn update(&self, f: impl FnOnce(&mut T))
    where
        T: 'scope,
    {
        self.write.update(f)
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        T: 'scope,
        O: ?Sized + 'scope,
        F: Fn(&T) -> &O + 'scope,
    {
        SignalSlice::new(self, getter)
    }
}

impl<'scope, 'run, T: 'scope> Signal<'scope, 'run, T> {
    pub(crate) fn from_rx(rx: Rx<'scope, 'run, T, RxValueKind>) -> Self {
        Self { rx }
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.rx.get()
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.rx.with(f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.rx.with_untracked(f)
    }

    pub fn is_constant(&self) -> bool {
        self.rx.is_constant()
    }

    pub fn is_alive(&self) -> bool {
        self.rx.is_alive()
    }

    pub fn into_rx(self) -> Rx<'scope, 'run, T> {
        self.rx
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        O: ?Sized + 'scope,
        F: Fn(&T) -> &O + 'scope,
    {
        SignalSlice::new(self, getter)
    }
}

impl<'scope, 'run, T: 'scope> From<ReadSignal<'scope, 'run, T>> for Signal<'scope, 'run, T> {
    fn from(signal: ReadSignal<'scope, 'run, T>) -> Self {
        Self::from_rx(Rx::from_signal(signal))
    }
}

impl<'scope, 'run, T: 'scope> From<RwSignal<'scope, 'run, T>> for Signal<'scope, 'run, T> {
    fn from(signal: RwSignal<'scope, 'run, T>) -> Self {
        signal.read.into()
    }
}

impl<'scope, 'run, T: 'scope> From<Memo<'scope, 'run, T>> for Signal<'scope, 'run, T> {
    fn from(memo: Memo<'scope, 'run, T>) -> Self {
        Self::from_rx(Rx::from_memo(memo))
    }
}

impl<'scope, 'run, T: 'scope> From<StoredValue<'scope, 'run, T>> for Signal<'scope, 'run, T> {
    fn from(stored: StoredValue<'scope, 'run, T>) -> Self {
        Self::from_rx(Rx::from_stored(stored))
    }
}

#[allow(dead_code)]
type _SignalMarker<T> = PhantomData<T>;
