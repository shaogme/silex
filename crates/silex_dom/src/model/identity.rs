/// Stable identity attached to every backend instance and every opaque handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BackendId(u64);

impl BackendId {
    pub(crate) fn fresh() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::BackendId;

    #[test]
    fn backend_ids_are_unique() {
        assert_ne!(BackendId::fresh(), BackendId::fresh());
    }
}
