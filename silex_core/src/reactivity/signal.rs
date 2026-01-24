use std::marker::PhantomData;
use std::panic::Location;

use silex_reactivity::NodeId;

use crate::reactivity::Memo;
use crate::traits::*;

// --- Signal 信号 Enum ---

#[derive(Debug)]
pub enum Signal<T: 'static> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

impl<T: Clone + 'static> Signal<T> {
    pub fn derive(f: impl Fn() -> T + 'static) -> Self {
        let id = silex_reactivity::register_derived(move || Box::new(f()));
        Signal::Derived(id, PhantomData)
    }
}

impl<T> DefinedAt for Signal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for Signal<T> {
    fn is_disposed(&self) -> bool {
        match self {
            Signal::Read(s) => s.is_disposed(),
            Signal::Derived(id, _) => !silex_reactivity::is_signal_valid(*id),
        }
    }
}

impl<T: 'static> Track for Signal<T> {
    fn track(&self) {
        match self {
            Signal::Read(s) => s.track(),
            Signal::Derived(id, _) => {
                // Run the derived function to track its dependencies
                let _ = silex_reactivity::run_derived::<T>(*id);
            }
        }
    }
}

impl<T: 'static> WithUntracked for Signal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        match self {
            Signal::Read(s) => s.try_with_untracked(fun),
            Signal::Derived(id, _) => {
                let val = untrack(|| silex_reactivity::run_derived::<T>(*id))?;
                Some(fun(&val))
            }
        }
    }
}

impl<T: Clone + 'static> Accessor<T> for Signal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + 'static> From<T> for Signal<T> {
    fn from(value: T) -> Self {
        let (read, _) = signal(value);
        Signal::Read(read)
    }
}

impl From<&str> for Signal<String> {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl<T: 'static> From<ReadSignal<T>> for Signal<T> {
    fn from(s: ReadSignal<T>) -> Self {
        Signal::Read(s)
    }
}

impl<T: 'static> From<RwSignal<T>> for Signal<T> {
    fn from(s: RwSignal<T>) -> Self {
        Signal::Read(s.read)
    }
}

// --- ReadSignal ---

pub struct ReadSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for ReadSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReadSignal({:?})", self.id)
    }
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ReadSignal<T> {}

impl<T> DefinedAt for ReadSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for ReadSignal<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

impl<T> Track for ReadSignal<T> {
    fn track(&self) {
        silex_reactivity::track_signal(self.id);
    }
}

impl<T: 'static> WithUntracked for ReadSignal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_signal_untracked(self.id, fun)
    }
}

impl<T: Clone + 'static> Accessor<T> for ReadSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: 'static + Clone> ReadSignal<T> {
    pub fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        Memo::new(move |_| f(self.get()))
    }
}

// Fluent API Extensions for ReadSignal
impl<T: Clone + 'static + PartialEq> ReadSignal<T> {
    pub fn eq<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() == other)
    }

    pub fn ne<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() != other)
    }
}

impl<T: Clone + 'static + PartialOrd> ReadSignal<T> {
    pub fn gt<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() > other)
    }

    pub fn lt<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() < other)
    }

    pub fn ge<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() >= other)
    }

    pub fn le<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() <= other)
    }
}

// --- WriteSignal ---

pub struct WriteSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for WriteSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WriteSignal({:?})", self.id)
    }
}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for WriteSignal<T> {}

impl<T> DefinedAt for WriteSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for WriteSignal<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

impl<T> Notify for WriteSignal<T> {
    fn notify(&self) {
        silex_reactivity::notify_signal(self.id);
    }
}

impl<T: 'static> UpdateUntracked for WriteSignal<T> {
    type Value = T;

    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_update_signal_silent(self.id, fun)
    }
}

impl<T: 'static> WriteSignal<T> {
    pub fn setter(self, value: T) -> impl Fn() + Clone
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    pub fn updater<F>(self, f: F) -> impl Fn() + Clone
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

impl<T: 'static> Update for WriteSignal<T> {
    type Value = T;

    fn try_maybe_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U> {
        let (did_update, val) = self.try_update_untracked(fun)?;
        if did_update {
            self.notify();
        }
        Some(val)
    }
}

// --- RwSignal ---

pub struct RwSignal<T: 'static> {
    pub read: ReadSignal<T>,
    pub write: WriteSignal<T>,
}

impl<T> Clone for RwSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for RwSignal<T> {}

impl<T: 'static> RwSignal<T> {
    pub fn new(value: T) -> Self {
        let (read, write) = signal(value);
        RwSignal { read, write }
    }

    pub fn read_signal(&self) -> ReadSignal<T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<T> {
        self.write
    }

    pub fn split(&self) -> (ReadSignal<T>, WriteSignal<T>) {
        (self.read, self.write)
    }

    pub fn from_parts(read: ReadSignal<T>, write: WriteSignal<T>) -> Self {
        Self { read, write }
    }

    pub fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
        T: Clone,
    {
        self.read.map(f)
    }

    pub fn setter(self, value: T) -> impl Fn() + Clone
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    pub fn updater<F>(self, f: F) -> impl Fn() + Clone
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

impl<T: 'static> DefinedAt for RwSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T: 'static> IsDisposed for RwSignal<T> {
    fn is_disposed(&self) -> bool {
        self.read.is_disposed()
    }
}

impl<T: 'static> Track for RwSignal<T> {
    fn track(&self) {
        self.read.track();
    }
}

impl<T: 'static> Notify for RwSignal<T> {
    fn notify(&self) {
        self.write.notify();
    }
}

impl<T: 'static> WithUntracked for RwSignal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.read.try_with_untracked(fun)
    }
}

impl<T: 'static> UpdateUntracked for RwSignal<T> {
    type Value = T;

    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        self.write.try_update_untracked(fun)
    }
}

impl<T: 'static> Update for RwSignal<T> {
    type Value = T;

    fn try_maybe_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U> {
        self.write.try_maybe_update(fun)
    }
}

impl<T: Clone + 'static> Accessor<T> for RwSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

// --- Global Functions ---

pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let id = silex_reactivity::signal(value);
    (
        ReadSignal {
            id,
            marker: PhantomData,
        },
        WriteSignal {
            id,
            marker: PhantomData,
        },
    )
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    silex_reactivity::untrack(f)
}
