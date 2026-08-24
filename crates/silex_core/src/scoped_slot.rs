use std::fmt;

use crate::{SilexError, SilexResult};

/// A scope-owned, DOM-independent slot for arbitrary runtime data.
pub struct ScopedSlot<'scope, T> {
    inner: silex_reactivity::NodeRef<'scope, T>,
}

impl<'scope, T> Copy for ScopedSlot<'scope, T> {}

impl<'scope, T> Clone for ScopedSlot<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for ScopedSlot<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("ScopedSlot").finish_non_exhaustive()
    }
}

impl<'scope, T: 'scope> ScopedSlot<'scope, T> {
    pub(crate) fn from_inner(inner: silex_reactivity::NodeRef<'scope, T>) -> Self {
        Self { inner }
    }

    /// Read the current value without exposing any DOM-specific type.
    pub fn get(&self) -> SilexResult<Option<T>>
    where
        T: Clone,
    {
        self.inner.get().map_err(SilexError::fatal)
    }

    /// Replace the current value in this scope.
    pub fn set(&self, value: T) -> SilexResult<()> {
        self.inner.set(value).map_err(SilexError::fatal)
    }

    /// Clear the slot. Repeated clear operations retain the runtime's error
    /// semantics instead of silently recreating a disposed scope.
    pub fn clear(&self) -> SilexResult<()> {
        self.inner.clear().map_err(SilexError::fatal)
    }
}

#[cfg(test)]
mod tests {
    use super::ScopedSlot;
    use crate::Runtime;

    #[test]
    fn scoped_slot_reads_writes_and_clears_inside_owner_scope() {
        let mut runtime = Runtime::new();
        let owner = runtime.owner().expect("owner should start");

        owner.with_access(|access| {
            let slot: ScopedSlot<'_, String> = access
                .scoped_slot()
                .expect("slot allocation should succeed");
            assert_eq!(slot.get().expect("read should succeed"), None);
            slot.set(String::from("value"))
                .expect("write should succeed");
            assert_eq!(
                slot.get().expect("read should succeed"),
                Some("value".into())
            );
            slot.clear().expect("clear should succeed");
            assert_eq!(slot.get().expect("read should succeed"), None);
        });
    }
}
