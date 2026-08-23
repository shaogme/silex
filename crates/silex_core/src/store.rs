use crate::{
    reactivity::{ReactiveSource, Signal},
    traits::{RxGet, RxReadRef, RxWrite},
};

/// A writable reactive handle that can be used as a Store field.
///
/// Store fields are owned by the same scope as the generated Store handle.
/// The bound also keeps enough runtime provenance available for validating a
/// Store assembled from existing handles.
pub trait StoreField<'scope, T>:
    ReactiveSource<'scope, Owned = T>
    + RxGet<Owned = T>
    + RxReadRef<T>
    + RxWrite<Owned = T>
    + Copy
    + 'scope
where
    T: 'scope,
{
}

impl<'scope, T> StoreField<'scope, T> for Signal<'scope, T> where T: Clone + 'scope {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Runtime;
    use crate::traits::RxGet;

    fn assert_store_field<'scope, T, F>(_: F)
    where
        T: 'scope,
        F: StoreField<'scope, T>,
    {
    }

    #[test]
    fn signal_is_a_store_field() {
        let mut runtime = Runtime::new();

        runtime
            .with_transient(|owner| {
                let field = owner.signal(42).expect("signal should initialize");
                assert_store_field(field);
                assert_eq!(field.get().expect("field should be readable"), 42);
            })
            .expect("child scope should initialize");
    }
}
