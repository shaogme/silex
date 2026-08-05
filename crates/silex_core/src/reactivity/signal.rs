use crate::{
    Rx, RxValueKind, Scope,
    reactivity::{Memo, SignalSlice, StoredValue},
    traits::{RxBase, RxRead, RxValue},
};
use silex_reactivity::{
    ReactiveResult, ReadSignal as RawReadSignal, WriteSignal as RawWriteSignal,
};
use std::fmt;

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

/// High-level read capability for a signal node.
pub struct ReadSignal<'scope, T> {
    pub(crate) inner: RawReadSignal<'scope, T>,
    pub(crate) scope: Scope<'scope>,
}

/// High-level write capability for a signal node.
pub struct WriteSignal<'scope, T> {
    pub(crate) inner: RawWriteSignal<'scope, T>,
}

/// A paired read/write signal.
pub struct RwSignal<'scope, T> {
    pub(crate) read: ReadSignal<'scope, T>,
    pub(crate) write: WriteSignal<'scope, T>,
}

/// A read-only union of the typed high-level node wrappers.
pub struct Signal<'scope, T> {
    pub(crate) rx: Rx<'scope, T, RxValueKind>,
}

impl<'scope, T> Copy for ReadSignal<'scope, T> {}

impl<'scope, T> Clone for ReadSignal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for WriteSignal<'scope, T> {}

impl<'scope, T> Clone for WriteSignal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for RwSignal<'scope, T> {}

impl<'scope, T> Clone for RwSignal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for Signal<'scope, T> {}

impl<'scope, T> Clone for Signal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for ReadSignal<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadSignal").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for WriteSignal<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteSignal").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for RwSignal<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwSignal").finish_non_exhaustive()
    }
}

impl<T> fmt::Debug for Signal<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signal").finish_non_exhaustive()
    }
}

impl<'scope, T: 'scope> ReadSignal<'scope, T> {
    pub(crate) fn from_inner(inner: RawReadSignal<'scope, T>, scope: Scope<'scope>) -> Self {
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

    pub fn into_rx(self) -> Rx<'scope, T> {
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

impl<'scope, T: 'scope> WriteSignal<'scope, T> {
    pub(crate) fn from_inner(inner: RawWriteSignal<'scope, T>) -> Self {
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

impl<'scope, T> RwSignal<'scope, T> {
    pub(crate) fn from_parts(read: ReadSignal<'scope, T>, write: WriteSignal<'scope, T>) -> Self {
        Self { read, write }
    }

    pub fn read_signal(&self) -> ReadSignal<'scope, T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<'scope, T> {
        self.write
    }

    pub fn into_rx(self) -> Rx<'scope, T>
    where
        T: 'scope,
    {
        self.read.into_rx()
    }

    pub fn split(&self) -> (ReadSignal<'scope, T>, WriteSignal<'scope, T>) {
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

impl<'scope, T: 'scope> Signal<'scope, T> {
    pub(crate) fn from_rx(rx: Rx<'scope, T, RxValueKind>) -> Self {
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

    pub fn into_rx(self) -> Rx<'scope, T> {
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

impl<'scope, T: 'scope> From<ReadSignal<'scope, T>> for Signal<'scope, T> {
    fn from(signal: ReadSignal<'scope, T>) -> Self {
        Self::from_rx(Rx::from_signal(signal))
    }
}

impl<'scope, T: 'scope> From<RwSignal<'scope, T>> for Signal<'scope, T> {
    fn from(signal: RwSignal<'scope, T>) -> Self {
        signal.read.into()
    }
}

impl<'scope, T: 'scope> From<Memo<'scope, T>> for Signal<'scope, T> {
    fn from(memo: Memo<'scope, T>) -> Self {
        Self::from_rx(Rx::from_memo(memo))
    }
}

impl<'scope, T: 'scope> From<StoredValue<'scope, T>> for Signal<'scope, T> {
    fn from(stored: StoredValue<'scope, T>) -> Self {
        Self::from_rx(Rx::from_stored(stored))
    }
}
