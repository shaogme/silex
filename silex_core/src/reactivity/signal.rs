use std::marker::PhantomData;
use std::mem;
use std::panic::Location;
use std::ptr;

use silex_reactivity::{
    NodeId, get_debug_label, get_node_defined_at, is_signal_valid, notify_signal, register_derived,
    set_debug_label, signal as create_signal, store_value, track_signal, try_update_signal_silent,
    untrack as untrack_scoped,
};

use crate::reactivity::SignalSlice;
use crate::traits::*;
use crate::traits::{RxCloneData, RxData};
use crate::{Rx, RxValueKind};

/// 内部辅助函数：直接从运行时借用信号值。
/// 安全性：由 RxGuard 的生命周期和 Silex Arena 的地址稳定性保证。
pub(crate) unsafe fn rx_borrow_signal_unsafe<T: RxData>(id: NodeId) -> Option<&'static T> {
    // 1. 尝试作为响应式信号借用
    if let Some(v) = silex_reactivity::try_with_signal_untracked(id, |v: &T| unsafe {
        std::mem::transmute::<&T, &'static T>(v)
    }) {
        return Some(v);
    }
    // 2. 尝试作为静态存储值借用 (常量)
    silex_reactivity::try_with_stored_value(id, |v: &T| unsafe {
        std::mem::transmute::<&T, &'static T>(v)
    })
}

/// 内部辅助函数：直接从运行时借用 StoredValue。
unsafe fn rx_borrow_stored_value_unsafe<T: RxData>(id: NodeId) -> Option<&'static T> {
    silex_reactivity::try_with_stored_value(id, |v: &T| unsafe {
        std::mem::transmute::<&T, &'static T>(v)
    })
}

// --- Constant ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Constant<T>(pub T);

impl<T: RxData> RxValue for Constant<T> {
    type Value = T;
}

impl<T: RxData> RxBase for Constant<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        None
    }
    #[inline(always)]
    fn track(&self) {}
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        false
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        Some("Constant".to_string())
    }
}

impl<T: RxData> RxInternal for Constant<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        Some(RxGuard::Borrowed {
            value: &self.0,
            token: None,
        })
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        Some(fun(&self.0))
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        true
    }
}

impl<T: RxCloneData> IntoRx for Constant<T> {
    type RxType = Rx<Self, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx(self, PhantomData)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
}

impl<T: RxCloneData> crate::traits::IntoSignal for Constant<T> {
    #[inline(always)]
    fn into_signal(self) -> Signal<T> {
        Signal::derive(Box::new(move || self.get()))
    }
}

// --- DerivedPayload ---

#[derive(Clone, Copy)]
pub struct DerivedPayload<Deps, F> {
    pub(crate) deps: Deps,
    pub(crate) func: F,
}

impl<D: std::fmt::Debug, F> std::fmt::Debug for DerivedPayload<D, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedPayload")
            .field("deps", &self.deps)
            .field("func", &"Fn(...)")
            .finish()
    }
}

impl<D, F> DerivedPayload<D, F> {
    pub const fn new(deps: D, func: F) -> Self {
        Self { deps, func }
    }
}

// --- RxInternal for DerivedPayloads ---

// Unary / Map implementation
impl<S, F, U> RxValue for DerivedPayload<S, F>
where
    S: RxValue,
    F: Fn(&S::Value) -> U + 'static,
{
    type Value = U;
}

impl<S, F, U> RxBase for DerivedPayload<S, F>
where
    S: RxBase + RxInternal,
    F: Fn(&S::Value) -> U + 'static,
    U: 'static,
{
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        self.deps.id()
    }
    #[inline(always)]
    fn track(&self) {
        self.deps.track();
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.deps.is_disposed()
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.deps.defined_at()
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.deps.debug_name()
    }
}

impl<S, F, U> RxInternal for DerivedPayload<S, F>
where
    S: RxInternal,
    F: Fn(&S::Value) -> U + 'static,
    U: 'static,
{
    type ReadOutput<'a>
        = RxGuard<'a, U, U>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.deps
            .rx_try_with_untracked(|v| (self.func)(v))
            .map(RxGuard::Owned)
    }

    #[inline(always)]
    fn rx_try_with_untracked<URet>(&self, fun: impl FnOnce(&Self::Value) -> URet) -> Option<URet> {
        self.deps.rx_try_with_untracked(|v| {
            let u_val = (self.func)(v);
            fun(&u_val)
        })
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    fn rx_is_constant(&self) -> bool {
        self.deps.rx_is_constant()
    }
}

impl<S, F, U> IntoRx for DerivedPayload<S, F>
where
    S: RxInternal + Clone,
    F: Fn(&S::Value) -> U + 'static,
    U: RxData,
{
    type RxType = Rx<Self, RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx(self, PhantomData)
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.deps.rx_is_constant()
    }
}

impl<S, F, U> crate::traits::IntoSignal for DerivedPayload<S, F>
where
    S: RxRead + Clone + 'static,
    for<'a> S::ReadOutput<'a>: std::ops::Deref<Target = S::Value>,
    F: Fn(&S::Value) -> U + 'static,
    U: RxCloneData,
{
    #[inline(always)]
    fn into_signal(self) -> Signal<Self::Value> {
        use crate::traits::RxGet;
        Signal::derive(Box::new(move || self.get()))
    }
}

// --- OpPayload (Aggressive De-genericization) ---

#[derive(Clone, Copy)]
pub struct OpPayload<U, const N: usize> {
    pub inputs: [NodeId; N],
    pub read: unsafe fn(inputs: &[NodeId]) -> Option<U>,
    pub track: fn(inputs: &[NodeId]),
    pub is_constant: bool,
}

impl<U: RxData, const N: usize> std::fmt::Debug for OpPayload<U, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpPayload")
            .field("inputs", &&self.inputs)
            .field("read", &format_args!("{:p}", self.read as *const ()))
            .field("is_constant", &self.is_constant)
            .finish()
    }
}

impl<U: RxData, const N: usize> RxValue for OpPayload<U, N> {
    type Value = U;
}

impl<U: RxData, const N: usize> RxBase for OpPayload<U, N> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        None
    }
    #[inline(always)]
    fn track(&self) {
        (self.track)(&self.inputs);
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        for i in 0..N {
            if !is_signal_valid(self.inputs[i]) {
                return true;
            }
        }
        false
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        None
    }
}

impl<U: RxData, const N: usize> RxInternal for OpPayload<U, N> {
    type ReadOutput<'a>
        = RxGuard<'a, U, U>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        unsafe { (self.read)(&self.inputs).map(RxGuard::Owned) }
    }

    #[inline(always)]
    fn rx_try_with_untracked<URet>(&self, fun: impl FnOnce(&Self::Value) -> URet) -> Option<URet> {
        unsafe { (self.read)(&self.inputs).map(|v| fun(&v)) }
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    fn rx_is_constant(&self) -> bool {
        self.is_constant
    }
}

impl<U: RxData + Clone, const N: usize> IntoRx for OpPayload<U, N> {
    type RxType = Rx<Self, RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx(self, PhantomData)
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.is_constant
    }
}

impl<U: RxCloneData, const N: usize> crate::traits::IntoSignal for OpPayload<U, N> {
    #[inline(always)]
    fn into_signal(self) -> Signal<Self::Value> {
        use crate::traits::RxGet;
        Signal::derive(Box::new(move || self.get()))
    }
}

// Trampoline 辅助工具：用于在宏中生成虚函数表实例
pub mod op_trampolines {
    use super::*;

    pub fn track_inputs(inputs: &[NodeId]) {
        for &id in inputs {
            track_signal(id);
        }
    }
}

// --- Signal 信号 Enum ---

pub enum Signal<T> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
    StoredConstant(NodeId, PhantomData<T>),
    #[allow(missing_docs)] // Internal optimization detail
    InlineConstant(u64, PhantomData<T>),
}

impl<T: RxData> std::fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(s) => f.debug_tuple("Read").field(s).finish(),
            Self::Derived(id, _) => f.debug_tuple("Derived").field(id).finish(),
            Self::StoredConstant(id, _) => f.debug_tuple("StoredConstant").field(id).finish(),
            Self::InlineConstant(val, _) => f.debug_tuple("InlineConstant").field(val).finish(),
        }
    }
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

impl<T: RxData> RxValue for Signal<T> {
    type Value = T;
}

impl<T: RxData> RxBase for Signal<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        match self {
            Signal::Read(s) => Some(s.id),
            Signal::Derived(id, _) => Some(*id),
            Signal::StoredConstant(id, _) => Some(*id),
            Signal::InlineConstant(_, _) => None,
        }
    }

    #[inline(always)]
    fn track(&self) {
        if let Some(id) = self.id() {
            track_signal(id);
        }
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.id().map(|id| !is_signal_valid(id)).unwrap_or(false)
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.id().and_then(get_node_defined_at)
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        let name = self.id().and_then(get_debug_label);
        if name.is_none() && self.is_constant() {
            Some("Constant".to_string())
        } else {
            name
        }
    }
}

impl<T: RxData> RxInternal for Signal<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        match self {
            Signal::Read(s) => s.rx_read_untracked(),
            Signal::Derived(id, _) => {
                // 不进行 track，仅获取当前值
                unsafe {
                    rx_borrow_signal_unsafe::<T>(*id).map(|v| RxGuard::Borrowed {
                        value: v,
                        token: Some(crate::NodeRef::from_id(*id)),
                    })
                }
            }
            Signal::StoredConstant(id, _) => unsafe {
                rx_borrow_stored_value_unsafe::<T>(*id).map(|v| RxGuard::Borrowed {
                    value: v,
                    token: Some(crate::NodeRef::from_id(*id)),
                })
            },
            Signal::InlineConstant(val, _) => {
                let val = unsafe { Self::unpack_inline(*val) };
                Some(RxGuard::Owned(val))
            }
        }
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        match self {
            Signal::Read(s) => s.rx_try_with_untracked(fun),
            Signal::Derived(id, _) => unsafe { rx_borrow_signal_unsafe(*id).map(fun) },
            Signal::StoredConstant(id, _) => unsafe { rx_borrow_stored_value_unsafe(*id).map(fun) },
            Signal::InlineConstant(storage, _) => {
                let val = unsafe { Self::unpack_inline(*storage) };
                Some(fun(&val))
            }
        }
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        match self {
            Signal::Read(s) => s.rx_get_adaptive(),
            Signal::Derived(_, _) | Signal::StoredConstant(_, _) => self
                .rx_try_with_untracked(|v| {
                    use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
                    AdaptiveWrapper(v).maybe_clone()
                })
                .flatten(),
            Signal::InlineConstant(storage, _) => {
                let val = unsafe { Self::unpack_inline(*storage) };
                Some(val)
            }
        }
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<T: RxData> IntoRx for Signal<T> {
    type RxType = Rx<Self, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx(self, PhantomData)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<T: RxData> crate::traits::IntoSignal for Signal<T> {
    #[inline(always)]
    fn into_signal(self) -> Signal<Self::Value> {
        self
    }
}

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

impl<T: RxData> Signal<T> {
    #[track_caller]
    pub fn derive(f: Box<dyn Fn() -> T>) -> Self {
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

    pub fn node_id(&self) -> Option<NodeId> {
        match self {
            Signal::Read(s) => Some(s.id),
            Signal::Derived(id, _) => Some(*id),
            Signal::StoredConstant(id, _) => Some(*id),
            Signal::InlineConstant(_, _) => None,
        }
    }

    /// 确保信号具有 NodeId。
    /// 如果是内联常量，则会将其提升为存储常量。
    pub fn ensure_node_id(&self) -> NodeId {
        if let Some(id) = self.node_id() {
            id
        } else if let Signal::InlineConstant(storage, _) = self {
            // 安全性：InlineConstant 保证了 T 不实现 Drop 且大小合适
            let value = unsafe { Self::unpack_inline(*storage) };
            store_value(value)
        } else {
            unreachable!("Signal must be either Read, Derived, StoredConstant or InlineConstant")
        }
    }

    pub fn is_constant(&self) -> bool {
        matches!(
            self,
            Signal::StoredConstant(_, _) | Signal::InlineConstant(_, _)
        )
    }
}

impl<T: Default + RxCloneData> Default for Signal<T> {
    fn default() -> Self {
        T::default().into()
    }
}

impl<T: RxCloneData> Signal<T> {
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
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static,
    {
        SignalSlice::new(self, getter)
    }
}

// Note: GetUntracked and Get methods are now provided as default methods in the RxRead trait.

impl<T: RxCloneData> From<T> for Signal<T> {
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

impl<T: RxData> From<ReadSignal<T>> for Signal<T> {
    fn from(s: ReadSignal<T>) -> Self {
        Signal::Read(s)
    }
}

impl<T: RxData> From<RwSignal<T>> for Signal<T> {
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
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static, // O can be unsized (e.g. str)
        T: 'static,
    {
        SignalSlice::new(self, getter)
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! impl_signal_core_traits {
    ($($ty:ident),*) => {
        $(
            impl<T> std::fmt::Debug for $ty<T> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}({:?})", stringify!($ty), self.id)
                }
            }

            impl<T> Clone for $ty<T> {
                fn clone(&self) -> Self {
                    *self
                }
            }
            impl<T> Copy for $ty<T> {}

            impl<T> PartialEq for $ty<T> {
                fn eq(&self, other: &Self) -> bool {
                    self.id == other.id
                }
            }

            impl<T> Eq for $ty<T> {}

            impl<T> std::hash::Hash for $ty<T> {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    self.id.hash(state);
                }
            }
        )*
    };
}

impl_signal_core_traits!(ReadSignal);

// Note: GetUntracked and Get are now blanket-implemented via RxRead

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

impl_signal_core_traits!(WriteSignal);

impl<T: RxData> RxValue for WriteSignal<T> {
    type Value = T;
}

impl<T: RxData> RxBase for WriteSignal<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        Some(self.id)
    }
    #[inline(always)]
    fn track(&self) {
        track_signal(self.id);
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        !is_signal_valid(self.id)
    }
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        get_node_defined_at(self.id)
    }
    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        get_debug_label(self.id)
    }
}

impl<T: RxData> RxWrite for WriteSignal<T> {
    #[inline(always)]
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        try_update_signal_silent(self.id, fun)
    }

    #[inline(always)]
    fn rx_notify(&self) {
        notify_signal(self.id);
    }
}

// --- RwSignal ---

pub struct RwSignal<T> {
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

impl<T: RxData> RwSignal<T> {
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
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static, // O can be unsized (e.g. str)
    {
        SignalSlice::new(self, getter)
    }
}

impl<T: 'static> RxWrite for RwSignal<T> {
    #[inline(always)]
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        self.write.rx_try_update_untracked(fun)
    }

    #[inline(always)]
    fn rx_notify(&self) {
        self.write.rx_notify();
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

// 手动实现了 RxInternal，移除自动委托以避免冲突
// crate::impl_rx_delegate!(Signal, false);
crate::impl_rx_delegate!(ReadSignal, SignalID, false);
crate::impl_rx_delegate!(RwSignal, read, false);
// Constant 使用手动实现以获得更优性能
// crate::impl_rx_delegate!(Constant, true);

crate::impl_reactive_ops!(Signal);
crate::impl_reactive_ops!(ReadSignal);
crate::impl_reactive_ops!(RwSignal);
crate::impl_reactive_ops!(Constant);
