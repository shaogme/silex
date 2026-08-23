use crate::{
    OwnerAccess, Rx, SilexError, SilexResult,
    callback::map_callback_error,
    reactivity::{BorrowedReadGuard, ReadGuard, RxInner, RxReadGuard, SignalSlice, WriteGuard},
    traits::{
        ReactiveInput, RuntimeScoped, RxBase, RxFrom, RxGet, RxRead, RxReadRef, RxReadRefSource,
        RxValue, RxWrite,
    },
};
use silex_reactivity::{
    ReadSignal as RawReadSignal, Signal as RawSignal, WriteSignal as RawWriteSignal,
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
    type Owned = T;
}

impl<T> RxBase for Constant<T> {
    fn track(&self) -> SilexResult<()> {
        Ok(())
    }
}

impl<T> RxRead for Constant<T> {
    type ReadGuard<'a>
        = BorrowedReadGuard<'a, T>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
        Ok(BorrowedReadGuard::new(&self.0))
    }

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.read()
    }
}

impl<T> RxReadRefSource for Constant<T> {
    type ViewGuard<'a>
        = BorrowedReadGuard<'a, T>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read()
    }

    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read_untracked()
    }
}

impl<T: Clone> RxGet for Constant<T> {
    fn get_untracked(&self) -> SilexResult<Self::Owned> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Owned> {
        self.with(Clone::clone)
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
pub struct Signal<'owner, T> {
    pub(crate) read: ReadSignal<'owner, T>,
    pub(crate) write: WriteSignal<'owner, T>,
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

impl<'owner, T> PartialEq for Signal<'owner, T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
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

impl<'owner, T> Signal<'owner, T> {
    pub fn from_pair(pair: (ReadSignal<'owner, T>, WriteSignal<'owner, T>)) -> SilexResult<Self> {
        let (read, write) = pair;
        RawSignal::from_pair((read.inner, write.inner)).map_err(SilexError::fatal)?;
        Ok(Self { read, write })
    }

    pub fn into_pair(self) -> (ReadSignal<'owner, T>, WriteSignal<'owner, T>) {
        (self.read, self.write)
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

impl<'owner, T> From<Signal<'owner, T>> for ReadSignal<'owner, T> {
    fn from(signal: Signal<'owner, T>) -> Self {
        signal.read
    }
}

impl<'owner, T> From<Signal<'owner, T>> for WriteSignal<'owner, T> {
    fn from(signal: Signal<'owner, T>) -> Self {
        signal.write
    }
}

impl<'owner, T: 'owner> From<Signal<'owner, T>> for Rx<'owner, T> {
    fn from(signal: Signal<'owner, T>) -> Self {
        Rx::from_signal(signal.read)
    }
}

impl<'owner, T> RuntimeScoped for Rx<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for ReadSignal<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.signal(value.into()).map(Into::into)
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Signal<'owner, T> {
    type Owned = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.signal(value.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T> RuntimeScoped for ReadSignal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for WriteSignal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for Signal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.read.owner_access()
    }
}

impl<'owner, T> RxValue for ReadSignal<'owner, T> {
    type Owned = T;
}

impl<'owner, T> RxBase for ReadSignal<'owner, T> {
    fn track(&self) -> SilexResult<()> {
        self.inner.track().map_err(SilexError::fatal)
    }
}

impl<'owner, T> RxRead for ReadSignal<'owner, T> {
    type ReadGuard<'a>
        = ReadGuard<'owner, T>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.inner
            .read()
            .map(ReadGuard::new)
            .map_err(SilexError::fatal)
    }

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.inner
            .read_untracked()
            .map(ReadGuard::new)
            .map_err(SilexError::fatal)
    }
}

impl<'owner, T> RxReadRefSource for ReadSignal<'owner, T> {
    type ViewGuard<'a>
        = ReadGuard<'owner, T>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read()
    }

    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read_untracked()
    }
}

impl<'owner, T: Clone> RxGet for ReadSignal<'owner, T> {
    fn get_untracked(&self) -> SilexResult<Self::Owned> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Owned> {
        self.with(Clone::clone)
    }
}

impl<'owner, T> RxValue for WriteSignal<'owner, T> {
    type Owned = T;
}

impl<'owner, T> RxWrite for WriteSignal<'owner, T> {
    type WriteGuard<'a>
        = WriteGuard<'owner, T>
    where
        Self: 'a;

    fn write(&self) -> SilexResult<Self::WriteGuard<'_>> {
        self.inner
            .write()
            .map(WriteGuard::new)
            .map_err(SilexError::fatal)
    }

    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        self.inner.notify().map_err(SilexError::fatal)
    }
}

impl<'owner, T> RxValue for Signal<'owner, T> {
    type Owned = T;
}

impl<'owner, T> RxBase for Signal<'owner, T> {
    fn track(&self) -> SilexResult<()> {
        self.read.track()
    }
}

impl<'owner, T> RxRead for Signal<'owner, T> {
    type ReadGuard<'a>
        = ReadGuard<'owner, T>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.read.read()
    }

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.read.read_untracked()
    }
}

impl<'owner, T> RxReadRefSource for Signal<'owner, T> {
    type ViewGuard<'a>
        = ReadGuard<'owner, T>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read()
    }

    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read_untracked()
    }
}

impl<'owner, T: Clone> RxGet for Signal<'owner, T> {
    fn get_untracked(&self) -> SilexResult<Self::Owned> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Owned> {
        self.with(Clone::clone)
    }
}

impl<'owner, T> RxWrite for Signal<'owner, T> {
    type WriteGuard<'a>
        = WriteGuard<'owner, T>
    where
        Self: 'a;

    fn write(&self) -> SilexResult<Self::WriteGuard<'_>> {
        self.write.write()
    }

    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.write.rx_update_untracked(f)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        self.write.rx_notify()
    }
}

impl<'owner, T> RxValue for Rx<'owner, T> {
    type Owned = T;
}

impl<'owner, T> RxBase for Rx<'owner, T> {
    fn track(&self) -> SilexResult<()> {
        match &self.inner {
            RxInner::ReadSignal(signal) => signal.track().map_err(SilexError::fatal),
            RxInner::Computed(computed) => computed.track().map_err(map_callback_error),
            RxInner::Stored(stored) => stored.track().map_err(SilexError::fatal),
        }
    }
}

impl<'owner, T> RxRead for Rx<'owner, T> {
    type ReadGuard<'a>
        = RxReadGuard<'owner, T>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
        match &self.inner {
            RxInner::ReadSignal(signal) => signal
                .read()
                .map(ReadGuard::new)
                .map_err(SilexError::fatal)
                .map(RxReadGuard::ReadSignal),
            RxInner::Computed(computed) => computed
                .read()
                .map(ReadGuard::new)
                .map_err(map_callback_error)
                .map(RxReadGuard::Computed),
            RxInner::Stored(stored) => stored
                .read()
                .map(ReadGuard::new)
                .map_err(SilexError::fatal)
                .map(RxReadGuard::Stored),
        }
    }

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
        match &self.inner {
            RxInner::ReadSignal(signal) => signal
                .read_untracked()
                .map(ReadGuard::new)
                .map_err(SilexError::fatal)
                .map(RxReadGuard::ReadSignal),
            RxInner::Computed(computed) => computed
                .read_untracked()
                .map(ReadGuard::new)
                .map_err(map_callback_error)
                .map(RxReadGuard::Computed),
            RxInner::Stored(stored) => stored
                .read_untracked()
                .map(ReadGuard::new)
                .map_err(SilexError::fatal)
                .map(RxReadGuard::Stored),
        }
    }
}

impl<'owner, T> RxReadRefSource for Rx<'owner, T> {
    type ViewGuard<'a>
        = RxReadGuard<'owner, T>
    where
        Self: 'a;

    fn read_ref<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read()
    }

    fn read_ref_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        self.read_untracked()
    }
}

impl<'owner, T: Clone> RxGet for Rx<'owner, T> {
    fn get_untracked(&self) -> SilexResult<Self::Owned> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Owned> {
        self.with(Clone::clone)
    }
}

#[cfg(test)]
mod tests {
    use super::Signal;
    use crate::Runtime;
    use crate::traits::RxGet;

    #[test]
    fn test_signal_partial_eq() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let read1 = owner.signal(10).expect("signal should initialize");
                let write1 = read1;
                let read2 = owner.signal(10).expect("signal should initialize");

                assert_eq!(read1, read1);
                assert_ne!(read1, read2);
                assert_eq!(write1, write1);

                let rw1 = owner.signal(20).expect("signal should initialize");
                let rw2 = owner.signal(20).expect("signal should initialize");

                assert_eq!(rw1, rw1);
                assert_ne!(rw1, rw2);

                let rebuilt = Signal::from_pair((rw1.read_signal(), rw1.write_signal()))
                    .expect("signal pair should initialize");
                assert_eq!(rebuilt.get().expect("signal should be readable"), 20);

                let first = owner.signal(30).expect("first signal should initialize");
                let second = owner.signal(40).expect("second signal should initialize");
                assert!(Signal::from_pair((first.read_signal(), second.write_signal())).is_err());
            })
            .expect("child owner should initialize");
    }
}
