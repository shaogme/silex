use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    marker::PhantomData,
    panic::Location,
};

use silex_reactivity::{RawNodeId as NodeId, StoredId, get_debug_label, set_debug_label, store};

use crate::{
    Rx, RxValueKind,
    reactivity::Signal,
    traits::{
        IntoSignal, RxData,
        adaptive::{AdaptiveFallback, AdaptiveWrapper},
        *,
    },
};

// --- StoredValue ---

pub struct StoredValue<T> {
    pub(crate) id: StoredId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> Debug for StoredValue<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "StoredValue({:?})", self.id)
    }
}

impl<T> Clone for StoredValue<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for StoredValue<T> {}

impl<T> PartialEq for StoredValue<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for StoredValue<T> {}

impl<T: RxData> StoredValue<T> {
    pub fn new(value: T) -> Self {
        let id = store::create(value);
        Self {
            id,
            marker: PhantomData,
        }
    }

    // Kept for backward compat or ease of use
    pub fn set_untracked(&self, value: T) {
        RxWrite::set_untracked(self, value)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        RxGet::get_untracked(self)
    }

    pub fn with_name(self, name: impl Into<String>) -> Self {
        set_debug_label(self.id, name);
        self
    }
}

// Note: GetUntracked is now blanket-implemented via WithUntracked when T: Clone

impl<T: RxData> RxValue for StoredValue<T> {
    type Value = T;
}

impl<T: RxData> RxBase for StoredValue<T> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        Some(self.id.raw())
    }

    #[inline(always)]
    fn track(&self) {
        // StoredValue is non-reactive, no-op
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        !self.id.is_alive()
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        get_debug_label(self.id)
    }
}

impl<T: RxData> RxInternal for StoredValue<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        // SAFETY: 借用被立刻收窄回 `RxGuard<'_, T, T>`，其生命周期挂在 `&self` 上，
        // 因此它不会逃逸出调用方的表达式作用域。
        //
        // 残留风险（AUDIT P6 未闭环的部分）：句柄是 `Copy` 的，它的存活与节点的
        // 存活无关 —— 调用方若在持有 guard 期间 `dispose` 这个节点，仍会读到已释放
        // 的内存。彻底修复需要运行时级别的借用计数，见审查报告 P6。
        let val = unsafe { store::try_value_ref::<T>(self.id)? };
        Some(RxGuard::Borrowed {
            value: val,
            token: Some(self.id.raw()),
        })
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        store::try_with(self.id, fun).ok()
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| AdaptiveWrapper(v).maybe_clone())
            .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        true
    }
}

impl<T: RxData + 'static> IntoRx for StoredValue<T> {
    type RxType = Rx<T, RxValueKind>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::new_stored(self.id.raw())
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
}

impl<T: RxData> IntoSignal for StoredValue<T> {
    #[inline(always)]
    fn into_signal(self) -> Signal<T> {
        Signal::StoredConstant(self.id.raw(), PhantomData)
    }
}

impl<T: RxData> RxWrite for StoredValue<T> {
    #[inline(always)]
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        store::try_update(self.id, fun).ok()
    }

    #[inline(always)]
    fn rx_notify(&self) {
        // StoredValue is non-reactive, notify is a no-op
    }
}
