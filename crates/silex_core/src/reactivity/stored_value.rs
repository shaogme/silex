use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    marker::PhantomData,
    panic::Location,
};

use silex_reactivity::{
    NodeId, get_debug_label, is_stored_value_valid, set_debug_label, store_value,
    try_get_stored_value_ref, try_update_stored_value, try_with_stored_value,
};

use crate::{
    NodeRef, Rx, RxValueKind,
    reactivity::Signal,
    traits::{
        IntoSignal, RxData,
        adaptive::{AdaptiveFallback, AdaptiveWrapper},
        *,
    },
};

// --- StoredValue ---

pub struct StoredValue<T> {
    pub(crate) id: NodeId,
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
        let id = store_value(value);
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
        Some(self.id)
    }

    #[inline(always)]
    fn track(&self) {
        // StoredValue is non-reactive, no-op
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        !is_stored_value_valid(self.id)
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
        let val = try_get_stored_value_ref::<T>(self.id)?;
        Some(RxGuard::Borrowed {
            value: val,
            token: Some(NodeRef::from_id(self.id)),
        })
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        try_with_stored_value(self.id, fun)
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
        Rx::new_signal(self.id)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
}

impl<T: RxData> IntoSignal for StoredValue<T> {
    #[inline(always)]
    fn into_signal(self) -> Signal<T> {
        Signal::StoredConstant(self.id, PhantomData)
    }
}

impl<T: RxData> RxWrite for StoredValue<T> {
    #[inline(always)]
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        try_update_stored_value(self.id, fun)
    }

    #[inline(always)]
    fn rx_notify(&self) {
        // StoredValue is non-reactive, notify is a no-op
    }
}
