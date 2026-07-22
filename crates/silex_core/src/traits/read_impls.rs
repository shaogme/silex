use std::{marker::PhantomData, ops::Deref, panic::Location};

use silex_reactivity::{get_debug_label, get_node_defined_at};

use crate::{
    Rx, RxInner,
    reactivity::{NodeId, Signal, dispatch},
    traits::{
        IntoRx, IntoSignal, RxBase, RxCloneData, RxData, RxGet, RxGuard, RxInternal, RxRead,
        RxValue,
    },
    unwrap_rx,
};

mod delegate_macro;
mod primitives;
mod tuples;

pub use tuples::{create_tuple_n_rx, create_tuple2_rx};

impl<T: ?Sized + RxRead> RxGet for T
where
    T::Value: Clone + Sized,
    for<'a> T::ReadOutput<'a>: Deref<Target = T::Value>,
{
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.try_read_untracked().map(|v| (*v).clone())
    }

    #[track_caller]
    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked()
            .unwrap_or_else(|| unwrap_rx!(self)())
    }

    #[track_caller]
    fn try_get(&self) -> Option<Self::Value> {
        self.try_read().map(|v| (*v).clone())
    }

    #[track_caller]
    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(|| unwrap_rx!(self)())
    }
}

impl<T: ?Sized + RxInternal> RxRead for T {}

impl<T: RxData, M> RxValue for Rx<T, M> {
    type Value = T;
}

impl<T: RxData, M> RxBase for Rx<T, M> {
    fn id(&self) -> Option<NodeId> {
        self.inner.as_node_parts().map(|(id, _)| id)
    }

    fn track(&self) {
        if let Some((id, kind)) = self.inner.as_node_parts() {
            dispatch::track(id, kind);
        }
    }

    fn is_disposed(&self) -> bool {
        if let Some((id, kind)) = self.inner.as_node_parts() {
            dispatch::is_disposed(id, kind)
        } else {
            false
        }
    }

    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.id().and_then(get_node_defined_at)
    }

    fn debug_name(&self) -> Option<String> {
        self.id().and_then(get_debug_label)
    }
}

impl<T: RxData, M> RxInternal for Rx<T, M> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        match &self.inner {
            RxInner::InlineConstant(storage) => unsafe {
                Some(RxGuard::Owned(Rx::<T, M>::unpack_inline(*storage)))
            },
            _ => {
                let (id, kind) = self.inner.as_node_parts()?;
                unsafe { dispatch::rx_read_node_untracked(id, kind) }
            }
        }
    }

    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        match &self.inner {
            RxInner::InlineConstant(storage) => unsafe {
                let val = Rx::<T, M>::unpack_inline(*storage);
                Some(fun(&val))
            },
            _ => {
                let (id, kind) = self.inner.as_node_parts()?;
                dispatch::rx_try_with_node_untracked(id, kind, fun)
            }
        }
    }

    fn rx_is_constant(&self) -> bool {
        matches!(self.inner, RxInner::InlineConstant(_) | RxInner::Stored(_))
    }
}

impl<T: RxData, M> IntoRx for Rx<T, M>
where
    T: RxCloneData,
{
    type RxType = Self;

    fn into_rx(self) -> Self::RxType {
        self
    }

    fn is_constant(&self) -> bool {
        self.rx_is_constant()
    }
}

impl<T: RxData, M> IntoSignal for Rx<T, M>
where
    T: RxCloneData,
    M: 'static,
{
    fn into_signal(self) -> Signal<Self::Value>
    where
        Self: Sized,
    {
        match self.inner {
            RxInner::InlineConstant(storage) => Signal::InlineConstant(storage, PhantomData),
            RxInner::Signal(id) => Signal::Derived(id, crate::RxNodeKind::Signal, PhantomData),
            RxInner::Closure(id) => Signal::Derived(id, crate::RxNodeKind::Closure, PhantomData),
            RxInner::Op(id) => Signal::Derived(id, crate::RxNodeKind::Op, PhantomData),
            RxInner::Stored(id) => Signal::StoredConstant(id, PhantomData),
        }
    }
}
