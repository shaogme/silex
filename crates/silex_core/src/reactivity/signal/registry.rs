use crate::{prelude::Signal, reactivity::SignalSlice, traits::*};
use silex_reactivity::{
    RawNodeId as NodeId, SignalId, get_debug_label, get_node_defined_at,
    scope::untrack as untrack_scoped, set_debug_label, signal,
};
use std::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    panic::Location,
};

// --- ReadSignal ---

/// 只读句柄。
///
/// id 是**擦除**的：`ReadSignal` 是“可读节点”的联合体 —— signal、memo、
/// derived 都能塞进来（`From<Memo<T>> for Signal<T>` 就是这么用的）。
/// 写入侧的 [`WriteSignal`] 才有静态种类可言，它拿的是 `SignalId`。
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
        O: ?Sized + 'static,
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
            const _: () = {
                use std::fmt::{Debug, Formatter, Result};
                use std::hash::{Hash, Hasher};

                impl<T> Debug for $ty<T> {
                    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
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

                impl<T> Hash for $ty<T> {
                    fn hash<H: Hasher>(&self, state: &mut H) {
                        self.id.hash(state);
                    }
                }
            };
        )*
    };
}

impl_signal_core_traits!(ReadSignal);

// --- WriteSignal ---

pub struct WriteSignal<T> {
    pub(crate) id: SignalId,
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
        Some(self.id.raw())
    }
    #[inline(always)]
    fn track(&self) {
        signal::track(self.id);
    }
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        !self.id.is_alive()
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
        signal::try_update_silent(self.id, fun).ok()
    }

    #[inline(always)]
    fn rx_notify(&self) {
        signal::notify(self.id);
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

impl<T> std::fmt::Debug for RwSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RwSignal")
            .field("read", &self.read)
            .field("write", &self.write)
            .finish()
    }
}

impl<T> Hash for RwSignal<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.read.hash(state);
        self.write.hash(state);
    }
}

impl<T: RxData> RwSignal<T> {
    #[track_caller]
    pub fn new(value: T) -> Self {
        let (read, write) = Signal::pair(value);
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
        O: ?Sized + 'static,
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

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    untrack_scoped(f)
}
