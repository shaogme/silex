use super::*;
use silex_reactivity::{create_scope};
use crate::traits::{RxGet, RxWrite};
use crate::reactivity::RwSignal;

#[test]
fn test_signal_derive_basic() {
    create_scope(|| {
        let rw = RwSignal::new(10);

        let derived = Signal::derive(Box::new(move || {
            rw.get() * 2
        }));

        // derived signals use register_derived which uses initialize_memo_raw,
        // and we can read them natively as if they are reactive values.
        assert_eq!(silex_reactivity::run_derived::<i32>(derived.node_id().unwrap()), Some(20));

        rw.set(15);
        assert_eq!(silex_reactivity::run_derived::<i32>(derived.node_id().unwrap()), Some(30));

    });
}

#[test]
fn test_signal_inline_constant_creation() {
    create_scope(|| {
        let inline_sig = Signal::from(42u32);

        assert!(matches!(inline_sig, Signal::InlineConstant(_, _)));
        assert_eq!(inline_sig.get(), 42u32);
        assert!(inline_sig.is_constant());
        assert_eq!(inline_sig.node_id(), None);
    });
}

#[test]
fn test_signal_stored_constant_creation() {
    create_scope(|| {
        let string_val = String::from("hello");
        let stored_sig = Signal::from(string_val);

        assert!(matches!(stored_sig, Signal::StoredConstant(_, _)));
        assert_eq!(stored_sig.get(), "hello".to_string());
        assert!(stored_sig.is_constant());
        assert!(stored_sig.node_id().is_some());
    });
}

#[test]
fn test_signal_ensure_node_id() {
    create_scope(|| {
        let inline_sig = Signal::from(42u32);

        assert!(matches!(inline_sig, Signal::InlineConstant(_, _)));
        assert_eq!(inline_sig.node_id(), None);

        let node_id = inline_sig.ensure_node_id();
        let stored_val = silex_reactivity::try_with_stored_value(node_id, |v: &u32| *v).unwrap();
        assert_eq!(stored_val, 42u32);
    });
}
