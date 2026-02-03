use std::marker::PhantomData;
use std::panic::Location;

use silex_reactivity::NodeId;

use crate::reactivity::Memo;
use crate::reactivity::SignalSlice;
use crate::traits::*;

// --- Constant ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Constant<T>(pub T);

impl<T> DefinedAt for Constant<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn debug_name(&self) -> Option<String> {
        Some("Constant".to_string())
    }
}

impl<T> IsDisposed for Constant<T> {
    fn is_disposed(&self) -> bool {
        false
    }
}

impl<T> Track for Constant<T> {
    fn track(&self) {}
}

impl<T> WithUntracked for Constant<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        Some(fun(&self.0))
    }
}

impl<T: Clone> GetUntracked for Constant<T> {
    type Value = T;

    fn try_get_untracked(&self) -> Option<T> {
        Some(self.0.clone())
    }
}

impl<T: Clone> Get for Constant<T> {
    type Value = T;

    fn try_get(&self) -> Option<T> {
        Some(self.0.clone())
    }
}

// --- Signal 信号 Enum ---

#[derive(Debug)]
pub enum Signal<T: 'static> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
    Constant(NodeId, PhantomData<T>),
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

impl<T: Clone + 'static> Signal<T> {
    pub fn derive(f: impl Fn() -> T + 'static) -> Self {
        let id = silex_reactivity::register_derived(move || f());
        Signal::Derived(id, PhantomData)
    }

    pub fn with_name(self, name: impl Into<String>) -> Self {
        match self {
            Signal::Read(s) => {
                s.with_name(name);
            }
            Signal::Derived(id, _) => silex_reactivity::set_debug_label(id, name),
            Signal::Constant(_, _) => {} // Constants usually don't need debug labels in the graph
        }
        self
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + Clone + 'static,
        O: ?Sized + 'static,
    {
        SignalSlice::new(self, getter)
    }
}

impl<T> DefinedAt for Signal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn debug_name(&self) -> Option<String> {
        match self {
            Signal::Read(s) => s.debug_name(),
            Signal::Derived(id, _) => silex_reactivity::get_debug_label(*id),
            Signal::Constant(_, _) => Some("Constant".to_string()),
        }
    }
}

impl<T> IsDisposed for Signal<T> {
    fn is_disposed(&self) -> bool {
        match self {
            Signal::Read(s) => s.is_disposed(),
            Signal::Derived(id, _) => !silex_reactivity::is_signal_valid(*id),
            Signal::Constant(_, _) => false,
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
            Signal::Constant(_, _) => {}
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
            Signal::Constant(id, _) => silex_reactivity::try_with_stored_value(*id, fun),
        }
    }
}

impl<T: Clone + 'static> GetUntracked for Signal<T> {
    type Value = T;
    fn try_get_untracked(&self) -> Option<T> {
        self.try_with_untracked(Clone::clone)
    }
}

impl<T: Clone + 'static> Get for Signal<T> {
    type Value = T;
    fn try_get(&self) -> Option<T> {
        self.try_with(Clone::clone)
    }
}

impl<T: Clone + 'static> Map for Signal<T> {
    type Value = T;

    fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        Memo::new(move |_| self.with(|val| f(val)))
    }
}

impl<T: Clone + 'static> From<T> for Signal<T> {
    fn from(value: T) -> Self {
        let id = silex_reactivity::store_value(value);
        Signal::Constant(id, PhantomData)
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

pub trait IntoSignal {
    type Value;
    type Signal: Get<Value = Self::Value>;

    fn into_signal(self) -> Self::Signal;
}

macro_rules! impl_into_signal_primitive {
    ($($t:ty),*) => {
        $(
            impl IntoSignal for $t {
                type Value = $t; // Self
                type Signal = Constant<$t>;

                fn into_signal(self) -> Self::Signal {
                    Constant(self)
                }
            }
        )*
    };
}

impl_into_signal_primitive!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

impl IntoSignal for String {
    type Value = String;
    type Signal = Constant<String>;

    fn into_signal(self) -> Self::Signal {
        Constant(self)
    }
}

impl IntoSignal for &str {
    type Value = String;
    type Signal = Constant<String>;

    fn into_signal(self) -> Self::Signal {
        Constant(self.to_string())
    }
}

impl<T: Clone + 'static> IntoSignal for Signal<T> {
    type Value = T;
    type Signal = Signal<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + 'static> IntoSignal for ReadSignal<T> {
    type Value = T;
    type Signal = ReadSignal<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + 'static> IntoSignal for RwSignal<T> {
    type Value = T;
    type Signal = RwSignal<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + PartialEq + 'static> IntoSignal for Memo<T> {
    type Value = T;
    type Signal = Memo<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + 'static> IntoSignal for Constant<T> {
    type Value = T;
    type Signal = Constant<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

// --- ReadSignal ---

pub struct ReadSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> ReadSignal<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        silex_reactivity::set_debug_label(self.id, name);
        self
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + Clone + 'static,
        O: ?Sized + 'static, // O can be unsized (e.g. str)
        T: 'static,
    {
        SignalSlice::new(self, getter)
    }
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

    fn debug_name(&self) -> Option<String> {
        silex_reactivity::get_debug_label(self.id)
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

impl<T: Clone + 'static> GetUntracked for ReadSignal<T> {
    type Value = T;
    fn try_get_untracked(&self) -> Option<T> {
        self.try_with_untracked(Clone::clone)
    }
}

impl<T: Clone + 'static> Get for ReadSignal<T> {
    type Value = T;
    fn try_get(&self) -> Option<T> {
        self.try_with(Clone::clone)
    }
}

impl<T: Clone + 'static> Map for ReadSignal<T> {
    type Value = T;

    fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        Memo::new(move |_| self.with(|val| f(val)))
    }
}

// --- WriteSignal ---

pub struct WriteSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> WriteSignal<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        silex_reactivity::set_debug_label(self.id, name);
        self
    }
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

    fn debug_name(&self) -> Option<String> {
        silex_reactivity::get_debug_label(self.id)
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

impl<T: Clone + 'static> SignalSetter for WriteSignal<T> {
    type Value = T;

    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static {
        move || self.set(value.clone())
    }
}

impl<T: 'static> SignalUpdater for WriteSignal<T> {
    type Value = T;

    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        F: Fn(&mut Self::Value) + Clone + 'static,
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

    pub fn with_name(self, name: impl Into<String>) -> Self {
        self.read.with_name(name);
        self
    }

    pub fn slice<O, F>(self, getter: F) -> SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + Clone + 'static,
        O: ?Sized + 'static, // O can be unsized (e.g. str)
    {
        SignalSlice::new(self, getter)
    }
}

impl<T: 'static> DefinedAt for RwSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn debug_name(&self) -> Option<String> {
        self.read.debug_name()
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

impl<T: Clone + 'static> GetUntracked for RwSignal<T> {
    type Value = T;
    fn try_get_untracked(&self) -> Option<T> {
        self.try_with_untracked(Clone::clone)
    }
}

impl<T: Clone + 'static> Get for RwSignal<T> {
    type Value = T;
    fn try_get(&self) -> Option<T> {
        self.try_with(Clone::clone)
    }
}

impl<T: Clone + 'static> Map for RwSignal<T> {
    type Value = T;

    fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        self.read.map(f)
    }
}

impl<T: Clone + 'static> SignalSetter for RwSignal<T> {
    type Value = T;

    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static {
        move || self.set(value.clone())
    }
}

impl<T: 'static> SignalUpdater for RwSignal<T> {
    type Value = T;

    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        F: Fn(&mut Self::Value) + Clone + 'static,
    {
        move || self.update(f.clone())
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

use crate::impl_reactive_ops;
impl_reactive_ops!(Signal);
impl_reactive_ops!(ReadSignal);
impl_reactive_ops!(RwSignal);
impl_reactive_ops!(Constant);
