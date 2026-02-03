use std::marker::PhantomData;
use std::panic::Location;

use silex_reactivity::NodeId;

use crate::traits::*;

// --- StoredValue ---

pub struct StoredValue<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for StoredValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredValue({:?})", self.id)
    }
}

impl<T> Clone for StoredValue<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for StoredValue<T> {}

impl<T: 'static> StoredValue<T> {
    pub fn new(value: T) -> Self {
        let id = silex_reactivity::store_value(value);
        Self {
            id,
            marker: PhantomData,
        }
    }

    // Kept for backward compat or ease of use
    // Kept for backward compat or ease of use
    pub fn set_untracked(&self, value: T) {
        SetUntracked::set_untracked(self, value)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        GetUntracked::get_untracked(self)
    }

    pub fn with_name(self, name: impl Into<String>) -> Self {
        silex_reactivity::set_debug_label(self.id, name);
        self
    }
}

impl<T> DefinedAt for StoredValue<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn debug_name(&self) -> Option<String> {
        silex_reactivity::get_debug_label(self.id)
    }
}

impl<T: 'static> WithUntracked for StoredValue<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_stored_value(self.id, fun)
    }
}

impl<T: 'static> IsDisposed for StoredValue<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

// StoredValue doesn't track reactively by design - it's a non-reactive storage
impl<T: 'static> Track for StoredValue<T> {
    fn track(&self) {
        // StoredValue is non-reactive, so tracking is a no-op
    }
}

// Note: GetUntracked is now blanket-implemented via WithUntracked when T: Clone

impl<T: 'static> UpdateUntracked for StoredValue<T> {
    type Value = T;

    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_update_stored_value(self.id, fun)
    }
}
