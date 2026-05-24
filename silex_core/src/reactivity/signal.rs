use std::marker::PhantomData;
use std::mem;
use std::panic::Location;
use std::ptr;

use silex_reactivity::{
    NodeId, get_debug_label, get_node_defined_at, register_derived, set_debug_label, store_value,
};

use crate::traits::*;
use crate::{Rx, RxValueKind};

mod derived;
mod ops;
mod registry;

pub use derived::*;
pub use ops::*;
pub use registry::*;

#[cfg(test)]
mod tests;

// --- Signal 信号 Enum ---

pub enum Signal<T> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
    StoredConstant(NodeId, PhantomData<T>),
    #[allow(missing_docs)] // Internal optimization detail
    InlineConstant(u64, PhantomData<T>),
}

impl<T: 'static> Signal<T> {
    pub fn pair(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
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

    fn track(&self) {
        match self {
            Signal::Read(s) => crate::reactivity::dispatch::track(s.id, crate::RxNodeKind::Signal),
            Signal::Derived(id, _) => {
                crate::reactivity::dispatch::track(*id, crate::RxNodeKind::Closure)
            }
            Signal::StoredConstant(id, _) => {
                crate::reactivity::dispatch::track(*id, crate::RxNodeKind::Stored)
            }
            Signal::InlineConstant(_, _) => {}
        }
    }

    fn is_disposed(&self) -> bool {
        match self {
            Signal::Read(s) => {
                crate::reactivity::dispatch::is_disposed(s.id, crate::RxNodeKind::Signal)
            }
            Signal::Derived(id, _) => {
                crate::reactivity::dispatch::is_disposed(*id, crate::RxNodeKind::Closure)
            }
            Signal::StoredConstant(id, _) => {
                crate::reactivity::dispatch::is_disposed(*id, crate::RxNodeKind::Stored)
            }
            Signal::InlineConstant(_, _) => false,
        }
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.id().and_then(get_node_defined_at)
    }

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
            Signal::Derived(id, _) => unsafe {
                crate::reactivity::dispatch::rx_read_node_untracked(*id, crate::RxNodeKind::Closure)
            },
            Signal::StoredConstant(id, _) => unsafe {
                crate::reactivity::dispatch::rx_read_node_untracked(*id, crate::RxNodeKind::Stored)
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
            Signal::Derived(id, _) => crate::reactivity::dispatch::rx_try_with_node_untracked(
                *id,
                crate::RxNodeKind::Closure,
                fun,
            ),
            Signal::StoredConstant(id, _) => {
                crate::reactivity::dispatch::rx_try_with_node_untracked(
                    *id,
                    crate::RxNodeKind::Stored,
                    fun,
                )
            }
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
    type RxType = Rx<T, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::new_signal(self.ensure_node_id())
    }
    fn is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<T: RxData> crate::traits::IntoSignal for Signal<T> {
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
        match self {
            Signal::Read(s) => s.id,
            Signal::Derived(id, _) => *id,
            Signal::StoredConstant(id, _) => *id,
            Signal::InlineConstant(storage, _) => {
                let value = unsafe { Self::unpack_inline(*storage) };
                store_value(value)
            }
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

    pub fn slice<O, F>(self, getter: F) -> crate::reactivity::SignalSlice<Self, F, O>
    where
        F: Fn(&T) -> &O + 'static,
        O: ?Sized + 'static,
    {
        crate::reactivity::SignalSlice::new(self, getter)
    }
}

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

// 手动实现了 RxInternal，移除自动委托以避免冲突
crate::impl_rx_delegate!(ReadSignal, SignalID, false);
crate::impl_rx_delegate!(RwSignal, read, false);

crate::impl_reactive_ops!(Signal);
crate::impl_reactive_ops!(ReadSignal);
crate::impl_reactive_ops!(RwSignal);
crate::impl_reactive_ops!(Constant);
