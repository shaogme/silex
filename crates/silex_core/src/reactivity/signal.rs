use crate::{
    RuntimeInputs, Rx, RxValueKind, Scope, SilexError, SilexResult,
    reactivity::{Memo, SignalSlice, StoredValue},
    traits::{RxBase, RxRead, RxValue},
};
use silex_reactivity::{
    ReactiveResult, ReadSignal as RawReadSignal, WriteSignal as RawWriteSignal,
    notify as raw_notify,
};
use std::fmt;

/// A plain value that has not yet been promoted into a runtime node.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
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
    fn track(&self) -> SilexResult<()> {
        Ok(())
    }
}

impl<T> RxRead for Constant<T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        Ok(f(&self.0))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        Ok(f(&self.0))
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
///
/// Access keeps the wrapped source kind intact. During final scope disposal,
/// raw signal-like sources follow their own inactive-node semantics, while a
/// `StoredValue`-backed instance follows the final-cleanup StoredValue path.
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

impl<'scope, T> PartialEq for ReadSignal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.scope == other.scope
    }
}

impl<'scope, T> Eq for ReadSignal<'scope, T> {}

impl<'scope, T> PartialEq for WriteSignal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<'scope, T> Eq for WriteSignal<'scope, T> {}

impl<'scope, T> PartialEq for RwSignal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}

impl<'scope, T> Eq for RwSignal<'scope, T> {}

impl<'scope, T> PartialEq for Signal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.rx == other.rx
    }
}

impl<'scope, T> Eq for Signal<'scope, T> {}

impl<'scope, T: 'scope> ReadSignal<'scope, T> {
    pub(crate) fn from_inner(inner: RawReadSignal<'scope, T>, scope: Scope<'scope>) -> Self {
        Self { inner, scope }
    }

    pub fn with_name(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn get(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        self.inner.get().map_err(SilexError::from)
    }

    pub fn get_untracked(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        self.inner.get_untracked().map_err(SilexError::from)
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::from)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(SilexError::from)
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

    pub fn set(&self, value: T) -> ReactiveResult<()> {
        self.inner.set(value)
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> ReactiveResult<U> {
        self.inner.update(f)
    }

    pub fn notify(&self) -> ReactiveResult<()> {
        raw_notify(&self.inner)
    }

    /// Return opaque runtime provenance for owner-bound validation.
    #[doc(hidden)]
    pub fn runtime_inputs(&self) -> RuntimeInputs {
        RuntimeInputs::single(self.inner.runtime_input())
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

    pub fn get(&self) -> SilexResult<T>
    where
        T: Clone + 'scope,
    {
        self.read.get()
    }

    pub fn set(&self, value: T) -> ReactiveResult<()>
    where
        T: 'scope,
    {
        self.write.set(value)
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) -> ReactiveResult<()>
    where
        T: 'scope,
    {
        self.write.update(f).map(|_| ())
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

    pub fn get(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        self.rx.get()
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.rx.with(f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.rx.with_untracked(f)
    }

    pub fn is_constant(&self) -> bool {
        self.rx.is_constant()
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

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn test_signal_partial_eq() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let (read1, write1) = scope.signal(10).expect("signal should initialize");
                let (read2, _write2) = scope.signal(10).expect("signal should initialize");

                assert_eq!(read1, read1);
                assert_ne!(read1, read2);
                assert_eq!(write1, write1);

                let rw1 = scope.rw_signal(20).expect("rw signal should initialize");
                let rw2 = scope.rw_signal(20).expect("rw signal should initialize");

                assert_eq!(rw1, rw1);
                assert_ne!(rw1, rw2);
            })
            .expect("child scope should initialize");
    }
}
