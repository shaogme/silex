use crate::{
    OwnerAccess, Rx, RxValueKind, SilexError, SilexResult,
    reactivity::{Computed, SignalSlice, StoredValue},
    traits::{RxRead, RxValue},
};
use silex_reactivity::{ReadSignal as RawReadSignal, WriteSignal as RawWriteSignal};
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

impl<T> RxRead for Constant<T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        Ok(f(&self.0))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        Ok(f(&self.0))
    }
}

/// High-level read capability for a signal node.
pub struct ReadSignal<'owner, T> {
    pub(crate) inner: RawReadSignal<'owner, T>,
    pub(crate) owner: OwnerAccess<'owner>,
}

/// High-level write capability for a signal node.
pub struct WriteSignal<'owner, T> {
    pub(crate) inner: RawWriteSignal<'owner, T>,
    pub(crate) owner: OwnerAccess<'owner>,
}

/// A paired read/write signal.
pub struct RwSignal<'owner, T> {
    pub(crate) read: ReadSignal<'owner, T>,
    pub(crate) write: WriteSignal<'owner, T>,
}

/// A read-only union of the typed high-level node wrappers.
///
/// Access keeps the wrapped source kind intact. During final owner disposal,
/// raw signal-like sources follow their own inactive-node semantics, while a
/// `StoredValue`-backed instance follows the final-cleanup StoredValue path.
pub struct Signal<'owner, T> {
    pub(crate) rx: Rx<'owner, T, RxValueKind>,
}

impl<'owner, T> Copy for ReadSignal<'owner, T> {}

impl<'owner, T> Clone for ReadSignal<'owner, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'owner, T> Copy for WriteSignal<'owner, T> {}

impl<'owner, T> Clone for WriteSignal<'owner, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'owner, T> Copy for RwSignal<'owner, T> {}

impl<'owner, T> Clone for RwSignal<'owner, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'owner, T> Copy for Signal<'owner, T> {}

impl<'owner, T> Clone for Signal<'owner, T> {
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

impl<'owner, T> PartialEq for ReadSignal<'owner, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.owner == other.owner
    }
}

impl<'owner, T> Eq for ReadSignal<'owner, T> {}

impl<'owner, T> PartialEq for WriteSignal<'owner, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<'owner, T> Eq for WriteSignal<'owner, T> {}

impl<'owner, T> PartialEq for RwSignal<'owner, T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}

impl<'owner, T> Eq for RwSignal<'owner, T> {}

impl<'owner, T> PartialEq for Signal<'owner, T> {
    fn eq(&self, other: &Self) -> bool {
        self.rx == other.rx
    }
}

impl<'owner, T> Eq for Signal<'owner, T> {}

impl<'owner, T: 'owner> ReadSignal<'owner, T> {
    pub(crate) fn from_inner(inner: RawReadSignal<'owner, T>, owner: OwnerAccess<'owner>) -> Self {
        Self { inner, owner }
    }

    pub fn with_name(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn get(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        self.inner.get().map_err(SilexError::fatal)
    }

    pub fn get_untracked(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        self.inner.get_untracked().map_err(SilexError::fatal)
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(SilexError::fatal)
    }

    pub fn into_rx(self) -> Rx<'owner, T> {
        Rx::from_signal(self)
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        O: ?Sized + 'owner,
        F: Fn(&T) -> &O + 'owner,
    {
        SignalSlice::new(self, getter)
    }
}

impl<'owner, T: 'owner> WriteSignal<'owner, T> {
    pub(crate) fn from_inner(inner: RawWriteSignal<'owner, T>, owner: OwnerAccess<'owner>) -> Self {
        Self { inner, owner }
    }

    pub fn with_name(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn set(&self, value: T) -> SilexResult<()> {
        self.inner.set(value).map_err(SilexError::fatal)
    }

    pub fn set_if_changed(&self, value: T) -> SilexResult<bool>
    where
        T: PartialEq,
    {
        self.inner.set_if_changed(value).map_err(SilexError::fatal)
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    pub fn notify(&self) -> SilexResult<()> {
        self.inner.notify().map_err(SilexError::fatal)
    }
}

impl<'owner, T> RwSignal<'owner, T> {
    pub(crate) fn from_parts(read: ReadSignal<'owner, T>, write: WriteSignal<'owner, T>) -> Self {
        Self { read, write }
    }

    pub fn read_signal(&self) -> ReadSignal<'owner, T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<'owner, T> {
        self.write
    }

    pub fn into_rx(self) -> Rx<'owner, T>
    where
        T: 'owner,
    {
        self.read.into_rx()
    }

    pub fn split(&self) -> (ReadSignal<'owner, T>, WriteSignal<'owner, T>) {
        (self.read, self.write)
    }

    pub fn get(&self) -> SilexResult<T>
    where
        T: Clone + 'owner,
    {
        self.read.get()
    }

    pub fn set(&self, value: T) -> SilexResult<()>
    where
        T: 'owner,
    {
        self.write.set(value)
    }

    pub fn set_if_changed(&self, value: T) -> SilexResult<bool>
    where
        T: PartialEq + 'owner,
    {
        self.write.set_if_changed(value)
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) -> SilexResult<()>
    where
        T: 'owner,
    {
        self.write.update(f)
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        T: 'owner,
        O: ?Sized + 'owner,
        F: Fn(&T) -> &O + 'owner,
    {
        SignalSlice::new(self, getter)
    }
}

impl<'owner, T: 'owner> Signal<'owner, T> {
    pub(crate) fn from_rx(rx: Rx<'owner, T, RxValueKind>) -> Self {
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

    pub fn into_rx(self) -> Rx<'owner, T> {
        self.rx
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        O: ?Sized + 'owner,
        F: Fn(&T) -> &O + 'owner,
    {
        SignalSlice::new(self, getter)
    }
}

impl<'owner, T: 'owner> From<ReadSignal<'owner, T>> for Signal<'owner, T> {
    fn from(signal: ReadSignal<'owner, T>) -> Self {
        Self::from_rx(Rx::from_signal(signal))
    }
}

impl<'owner, T: 'owner> From<RwSignal<'owner, T>> for Signal<'owner, T> {
    fn from(signal: RwSignal<'owner, T>) -> Self {
        signal.read.into()
    }
}

impl<'owner, T: 'owner> From<Computed<'owner, T>> for Signal<'owner, T> {
    fn from(computed: Computed<'owner, T>) -> Self {
        Self::from_rx(Rx::from_computed(computed))
    }
}

impl<'owner, T: 'owner> From<StoredValue<'owner, T>> for Signal<'owner, T> {
    fn from(stored: StoredValue<'owner, T>) -> Self {
        Self::from_rx(Rx::from_stored(stored))
    }
}

impl<'owner, T: 'owner> From<Rx<'owner, T>> for Signal<'owner, T> {
    fn from(rx: Rx<'owner, T>) -> Self {
        Self::from_rx(rx)
    }
}

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn test_signal_partial_eq() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let (read1, write1) = owner.signal(10).expect("signal should initialize");
                let (read2, _write2) = owner.signal(10).expect("signal should initialize");

                assert_eq!(read1, read1);
                assert_ne!(read1, read2);
                assert_eq!(write1, write1);

                let rw1 = owner.rw_signal(20).expect("rw signal should initialize");
                let rw2 = owner.rw_signal(20).expect("rw signal should initialize");

                assert_eq!(rw1, rw1);
                assert_ne!(rw1, rw2);
            })
            .expect("child owner should initialize");
    }
}
