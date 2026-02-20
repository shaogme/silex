use std::marker::PhantomData;
use std::mem;
use std::panic::Location;
use std::ptr;

use silex_reactivity::{
    NodeId, get_debug_label, get_node_defined_at, is_signal_valid, notify_signal, register_derived,
    set_debug_label, signal as create_signal, store_value, track_signal, try_update_signal_silent,
    try_with_signal_untracked, try_with_stored_value, untrack as untrack_scoped,
};

use crate::reactivity::SignalSlice;
use crate::traits::*;

// --- Constant ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

// --- Derived ---

#[derive(Clone, Copy)]
pub struct Derived<S, F> {
    pub(crate) source: S,
    pub(crate) f: F,
}

impl<S: std::fmt::Debug, F> std::fmt::Debug for Derived<S, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Derived")
            .field("source", &self.source)
            .field("f", &"Fn(...)")
            .finish()
    }
}

impl<S, F> Derived<S, F> {
    pub const fn new(source: S, f: F) -> Self {
        Self { source, f }
    }
}

impl<S: DefinedAt, F> DefinedAt for Derived<S, F> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.source.defined_at()
    }
}

impl<S: IsDisposed, F> IsDisposed for Derived<S, F> {
    fn is_disposed(&self) -> bool {
        self.source.is_disposed()
    }
}

impl<S: Track, F> Track for Derived<S, F> {
    fn track(&self) {
        self.source.track();
    }
}

impl<S, F, U> WithUntracked for Derived<S, F>
where
    S: WithUntracked,
    F: Fn(&S::Value) -> U,
{
    type Value = U;

    fn try_with_untracked<R>(&self, fun: impl FnOnce(&Self::Value) -> R) -> Option<R> {
        self.source.try_with_untracked(|val| {
            let mapped = (self.f)(val);
            fun(&mapped)
        })
    }
}

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

impl<S, F, U> IntoSignal for Derived<S, F>
where
    S: WithUntracked + Track + Clone + 'static,
    F: Fn(&S::Value) -> U + Clone + 'static,
    U: Clone + 'static,
{
    type Value = U;
    type Signal = Self;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

// --- ReactiveBinary ---

#[derive(Clone, Copy)]
pub struct ReactiveBinary<L, R, F> {
    pub(crate) lhs: L,
    pub(crate) rhs: R,
    pub(crate) f: F,
}

impl<L: std::fmt::Debug, R: std::fmt::Debug, F> std::fmt::Debug for ReactiveBinary<L, R, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactiveBinary")
            .field("lhs", &self.lhs)
            .field("rhs", &self.rhs)
            .field("f", &"Fn(...)")
            .finish()
    }
}

impl<L, R, F> ReactiveBinary<L, R, F> {
    pub const fn new(lhs: L, rhs: R, f: F) -> Self {
        Self { lhs, rhs, f }
    }
}

impl<L, R, F> DefinedAt for ReactiveBinary<L, R, F>
where
    L: DefinedAt,
    R: DefinedAt,
{
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.lhs.defined_at().or(self.rhs.defined_at())
    }
}

impl<L, R, F> IsDisposed for ReactiveBinary<L, R, F>
where
    L: IsDisposed,
    R: IsDisposed,
{
    fn is_disposed(&self) -> bool {
        self.lhs.is_disposed() || self.rhs.is_disposed()
    }
}

impl<L, R, F> Track for ReactiveBinary<L, R, F>
where
    L: Track,
    R: Track,
{
    fn track(&self) {
        self.lhs.track();
        self.rhs.track();
    }
}

impl<L, R, F, U> WithUntracked for ReactiveBinary<L, R, F>
where
    L: WithUntracked,
    R: WithUntracked,
    F: Fn(&L::Value, &R::Value) -> U,
{
    type Value = U;

    fn try_with_untracked<Res>(&self, fun: impl FnOnce(&Self::Value) -> Res) -> Option<Res> {
        self.lhs
            .try_with_untracked(|lhs_val| {
                self.rhs.try_with_untracked(|rhs_val| {
                    let res = (self.f)(lhs_val, rhs_val);
                    fun(&res)
                })
            })
            .flatten()
    }
}

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

impl<L, R, F, U> IntoSignal for ReactiveBinary<L, R, F>
where
    L: WithUntracked + Track + Clone + 'static,
    R: WithUntracked + Track + Clone + 'static,
    F: Fn(&L::Value, &R::Value) -> U + Clone + 'static,
    U: Clone + 'static,
{
    type Value = U;
    type Signal = Self;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

// --- Signal 信号 Enum ---

#[derive(Debug)]
pub enum Signal<T: 'static> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
    StoredConstant(NodeId, PhantomData<T>),
    #[allow(missing_docs)] // Internal optimization detail
    InlineConstant(u64, PhantomData<T>),
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

impl<T> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Read(l), Self::Read(r)) => l == r,
            (Self::Derived(l, _), Self::Derived(r, _)) => l == r,
            (Self::StoredConstant(l, _), Self::StoredConstant(r, _)) => l == r,
            (Self::InlineConstant(l, _), Self::InlineConstant(r, _)) => l == r,
            _ => false,
        }
    }
}

impl<T> Eq for Signal<T> {}

impl<T> std::hash::Hash for Signal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Read(s) => s.hash(state),
            Self::Derived(id, _) => id.hash(state),
            Self::StoredConstant(id, _) => id.hash(state),
            Self::InlineConstant(val, _) => val.hash(state),
        }
    }
}

// --- Generic Impl Block ---

impl<T: 'static> Signal<T> {
    #[track_caller]
    pub fn derive(f: impl Fn() -> T + 'static) -> Self {
        let id = register_derived(f);
        Signal::Derived(id, PhantomData)
    }

    /// Internal helper to try inlining a value
    fn try_inline(value: T) -> Option<Self> {
        // Can only inline if it fits in u64 and doesn't implement Drop
        #[allow(clippy::manual_is_variant_and)] // we want explicit check
        if mem::size_of::<T>() <= mem::size_of::<u64>() && !mem::needs_drop::<T>() {
            unsafe {
                let mut storage = 0u64;
                let src_ptr = &value as *const T as *const u8;
                let dst_ptr = &mut storage as *mut u64 as *mut u8;
                ptr::copy_nonoverlapping(src_ptr, dst_ptr, mem::size_of::<T>());
                // Value is not dropped because we checked !needs_drop, so we can just forget it
                mem::forget(value);
                Some(Signal::InlineConstant(storage, PhantomData))
            }
        } else {
            None
        }
    }

    /// Internal helper to unpack an inlined value
    unsafe fn unpack_inline(storage: u64) -> T {
        unsafe {
            let mut value = mem::MaybeUninit::<T>::uninit();
            let src_ptr = &storage as *const u64 as *const u8;
            let dst_ptr = value.as_mut_ptr() as *mut u8;
            ptr::copy_nonoverlapping(src_ptr, dst_ptr, mem::size_of::<T>());
            value.assume_init()
        }
    }
}

impl<T: Clone + 'static> Signal<T> {
    // derive moved to T: 'static block

    pub fn with_name(self, name: impl Into<String>) -> Self {
        match self {
            Signal::Read(s) => {
                s.with_name(name);
            }
            Signal::Derived(id, _) => set_debug_label(id, name),
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _) => {} // Constants usually don't need debug labels in the graph
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
        match self {
            Signal::Read(s) => s.defined_at(),
            Signal::Derived(id, _) => get_node_defined_at(*id),
            Signal::StoredConstant(id, _) => get_node_defined_at(*id),
            Signal::InlineConstant(_, _) => None,
        }
    }

    fn debug_name(&self) -> Option<String> {
        match self {
            Signal::Read(s) => s.debug_name(),
            Signal::Derived(id, _) => get_debug_label(*id),
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _) => {
                Some("Constant".to_string())
            }
        }
    }
}

impl<T> IsDisposed for Signal<T> {
    fn is_disposed(&self) -> bool {
        match self {
            Signal::Read(s) => s.is_disposed(),
            Signal::Derived(id, _) => !is_signal_valid(*id),
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _) => false,
        }
    }
}

impl<T: 'static> Track for Signal<T> {
    fn track(&self) {
        match self {
            Signal::Read(s) => s.track(),
            Signal::Derived(id, _) => {
                track_signal(*id);
            }
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _) => {}
        }
    }
}

impl<T: 'static> WithUntracked for Signal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        match self {
            Signal::Read(s) => s.try_with_untracked(fun),
            Signal::Derived(id, _) => try_with_signal_untracked(*id, fun),
            Signal::StoredConstant(id, _) => try_with_stored_value(*id, fun),
            Signal::InlineConstant(val, _) => {
                // Unsafe: we verified safety conditions on creation
                let val = unsafe { Self::unpack_inline(*val) };
                Some(fun(&val))
            }
        }
    }
}

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

impl<T: Clone + 'static> From<T> for Signal<T> {
    #[track_caller]
    fn from(value: T) -> Self {
        if let Some(inline) = Self::try_inline(value.clone()) {
            return inline;
        }
        let id = store_value(value);
        Signal::StoredConstant(id, PhantomData)
    }
}

impl From<&str> for Signal<String> {
    #[track_caller]
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

impl<T> ReadSignal<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        set_debug_label(self.id, name);
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

impl<T> PartialEq for ReadSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for ReadSignal<T> {}

impl<T> std::hash::Hash for ReadSignal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> DefinedAt for ReadSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        get_node_defined_at(self.id)
    }

    fn debug_name(&self) -> Option<String> {
        get_debug_label(self.id)
    }
}

impl<T> IsDisposed for ReadSignal<T> {
    fn is_disposed(&self) -> bool {
        !is_signal_valid(self.id)
    }
}

impl<T> Track for ReadSignal<T> {
    fn track(&self) {
        track_signal(self.id);
    }
}

impl<T: 'static> WithUntracked for ReadSignal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        try_with_signal_untracked(self.id, fun)
    }
}

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

// --- WriteSignal ---

pub struct WriteSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> WriteSignal<T> {
    pub fn with_name(self, name: impl Into<String>) -> Self {
        set_debug_label(self.id, name);
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

impl<T> PartialEq for WriteSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for WriteSignal<T> {}

impl<T> std::hash::Hash for WriteSignal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> DefinedAt for WriteSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        get_node_defined_at(self.id)
    }

    fn debug_name(&self) -> Option<String> {
        get_debug_label(self.id)
    }
}

impl<T> IsDisposed for WriteSignal<T> {
    fn is_disposed(&self) -> bool {
        !is_signal_valid(self.id)
    }
}

impl<T> Notify for WriteSignal<T> {
    fn notify(&self) {
        notify_signal(self.id);
    }
}

impl<T: 'static> UpdateUntracked for WriteSignal<T> {
    type Value = T;

    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        try_update_signal_silent(self.id, fun)
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

impl<T> PartialEq for RwSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}

impl<T> Eq for RwSignal<T> {}

impl<T> std::hash::Hash for RwSignal<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.read.hash(state);
        self.write.hash(state);
    }
}

impl<T: 'static> RwSignal<T> {
    #[track_caller]
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
        self.read.defined_at()
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

// Note: GetUntracked and Get are now blanket-implemented via WithUntracked + Track

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

#[track_caller]
pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let id = create_signal(value);
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
    untrack_scoped(f)
}

use crate::impl_reactive_ops;
impl_reactive_ops!(Signal);
impl_reactive_ops!(ReadSignal);
impl_reactive_ops!(RwSignal);
impl_reactive_ops!(Constant);
