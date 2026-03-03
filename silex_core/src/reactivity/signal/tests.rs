use super::*;
use std::rc::Rc;
use silex_reactivity::create_scope;

#[test]
fn test_inline_constants() {
    create_scope(|| {
        let signal_i32 = Signal::from(42i32);
        assert!(matches!(signal_i32, Signal::InlineConstant(_, _)));
        assert_eq!(signal_i32.rx_get_adaptive(), Some(42i32));

        let signal_bool = Signal::from(true);
        assert!(matches!(signal_bool, Signal::InlineConstant(_, _)));
        assert_eq!(signal_bool.rx_get_adaptive(), Some(true));

        let signal_u64 = Signal::from(u64::MAX);
        assert!(matches!(signal_u64, Signal::InlineConstant(_, _)));
        assert_eq!(signal_u64.rx_get_adaptive(), Some(u64::MAX));
    });
}

#[test]
fn test_non_inline_constants() {
    create_scope(|| {
        // String needs drop
        let s = String::from("hello");
        let signal_string = Signal::from(s.clone());
        assert!(matches!(signal_string, Signal::StoredConstant(_, _)));

        // Vec needs drop
        let v = vec![1, 2, 3];
        let signal_vec = Signal::from(v.clone());
        assert!(matches!(signal_vec, Signal::StoredConstant(_, _)));

        // [u8; 16] doesn't need drop but size > 8
        let large_array: [u8; 16] = [0; 16];
        let signal_array = Signal::from(large_array);
        assert!(matches!(signal_array, Signal::StoredConstant(_, _)));

        // Rc needs drop
        let rc = Rc::new(42);
        let signal_rc = Signal::from(rc.clone());
        assert!(matches!(signal_rc, Signal::StoredConstant(_, _)));
    });
}

#[test]
fn test_is_constant() {
    create_scope(|| {
        let inline = Signal::from(42);
        assert!(inline.is_constant());

        let stored = Signal::from(String::from("test"));
        assert!(stored.is_constant());
    });
}

#[test]
fn test_ensure_node_id() {
    create_scope(|| {
        // Stored constant already has an ID
        let stored = Signal::from(String::from("test"));
        let id1 = stored.ensure_node_id();
        assert_eq!(stored.id(), Some(id1));

        // Inline constant gets converted/promoted to have an ID
        let inline = Signal::from(42);
        assert_eq!(inline.id(), None);
        let id2 = inline.ensure_node_id();
        // The original inline signal still doesn't have an ID conceptually,
        // but ensure_node_id allocates one in the runtime graph
        assert!(id2.index > 0);
    });
}

#[test]
fn test_derive() {
    create_scope(|| {
        // Simple derived signal
        let d = Signal::derive(Box::new(|| 42));
        assert!(matches!(d, Signal::Derived(_, _)));
        assert!(!d.is_constant());

        // Ensure evaluating the derived value directly evaluates to 42
        // We'll read the node untracked using standard core routines:
        assert_eq!(silex_reactivity::run_derived::<i32>(d.ensure_node_id()), Some(42));
    });
}
