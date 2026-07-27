use super::*;
use crate::reactivity::{RwSignal, dispatch};
use crate::traits::{IntoRx, RxGet, RxWrite};
use silex_reactivity::scope::create as create_scope;
use std::rc::Rc;

#[test]
fn test_signal_derive_basic() {
    create_scope(|| {
        let rw = RwSignal::new(10);

        let derived = Signal::derive(Box::new(move || rw.get() * 2));

        // derived signals use register_derived which uses initialize_memo_raw,
        // and we can read them natively as if they are reactive values.
        assert_eq!(
            silex_reactivity::signal::try_get::<i32>(derived.raw_id().unwrap()),
            Ok(20)
        );

        rw.set(15);
        assert_eq!(
            silex_reactivity::signal::try_get::<i32>(derived.raw_id().unwrap()),
            Ok(30)
        );
    });
}

#[test]
fn test_signal_inline_constant_creation() {
    create_scope(|| {
        let inline_sig = Signal::from(42u32);

        assert!(matches!(inline_sig, Signal::InlineConstant(_, _)));
        assert_eq!(inline_sig.get(), 42u32);
        assert!(inline_sig.is_constant());
        assert_eq!(inline_sig.raw_id(), None);
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
        assert!(stored_sig.raw_id().is_some());
    });
}

#[test]
fn test_signal_ensure_raw_id() {
    create_scope(|| {
        let inline_sig = Signal::from(42u32);

        assert!(matches!(inline_sig, Signal::InlineConstant(_, _)));
        assert_eq!(inline_sig.raw_id(), None);

        let raw_id = inline_sig.ensure_raw_id();
        let stored_val = silex_reactivity::store::try_with(
            silex_reactivity::StoredId::from_raw_unchecked(raw_id),
            |v: &u32| *v,
        )
        .unwrap();
        assert_eq!(stored_val, 42u32);
    });
}

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
fn test_ensure_raw_id() {
    create_scope(|| {
        // Stored constant already has an ID
        let stored = Signal::from(String::from("test"));
        let id1 = stored.ensure_raw_id();
        assert_eq!(stored.raw_id(), Some(id1));

        // Inline constant gets converted/promoted to have an ID
        let inline = Signal::from(42);
        assert_eq!(inline.raw_id(), None);
        let id2 = inline.ensure_raw_id();
        // The original inline signal still doesn't have an ID conceptually,
        // but ensure_raw_id allocates one in the runtime graph
        assert_ne!(id2, id1, "提升出来的常量节点必须是一个新句柄");
    });
}

#[test]
fn test_ensure_raw_id_survives_child_scope_dispose() {
    create_scope(|| {
        let inline = Signal::from(42);
        let mut raw_id = None;
        let child_scope = silex_reactivity::scope::create(|| {
            raw_id = Some(inline.ensure_raw_id());
        });
        silex_reactivity::scope::dispose(child_scope);
        let rx = Rx::<i32, RxValueKind>::new_stored(
            silex_reactivity::StoredId::from_raw_unchecked(raw_id.unwrap()),
        );
        assert_eq!(rx.get(), 42);

        let mut stored_rx = None;
        let child_scope2 = silex_reactivity::scope::create(|| {
            let sig = Signal::from("detached_test".to_string());
            stored_rx = Some(sig.into_rx());
        });
        silex_reactivity::scope::dispose(child_scope2);
        assert_eq!(stored_rx.unwrap().get(), "detached_test".to_string());
    });
}

#[test]
fn test_derive() {
    create_scope(|| {
        // Simple derived signal
        let d = Signal::derive(Box::new(|| 42));
        assert!(matches!(d, Signal::Derived(..)));
        assert!(!d.is_constant());

        // Ensure evaluating the derived value directly evaluates to 42
        // We'll read the node untracked using standard core routines:
        assert_eq!(
            silex_reactivity::signal::try_get::<i32>(d.ensure_raw_id()),
            Ok(42)
        );
    });
}

#[test]
fn test_rw_signal_new() {
    create_scope(|| {
        let rw = RwSignal::new(42);
        assert_eq!(rw.get(), 42);

        rw.set(43);
        assert_eq!(rw.get(), 43);

        let read = rw.read_signal();
        assert_eq!(read.get(), 43);

        let write = rw.write_signal();
        write.set(44);
        assert_eq!(rw.get(), 44);

        let (r, w) = rw.split();
        assert_eq!(r, read);
        assert_eq!(w, write);
    });
}

/// `Signal::derive` 必须能通过**普通的读取 trait** 读出来。
///
/// 它从前被标成 `RxNodeKind::Closure`，而 `register_derived` 建的是一个响应式
/// 节点（住在 `reactive` 表里），`Closure` 那条分支去查的却是 `extras` 表 ——
/// 于是 `RxRead::get()` 恒为 `None`。整个 crate 里唯一覆盖它的用例是直接调
/// `silex_reactivity` 的底层读取绕过了这条分发，所以没被发现。
#[test]
fn a_derived_signal_reads_through_the_normal_trait() {
    create_scope(|| {
        let rw = RwSignal::new(10);
        let derived = Signal::derive(Box::new(move || rw.get() * 2));

        assert_eq!(derived.get(), 20, "派生值必须能通过 RxRead 读出来");

        rw.set(15);
        assert_eq!(derived.get(), 30, "上游变化后必须读到重算之后的值");
    });
}

/// `StoredValue` 转成 `Rx` 之后不能被误判成“已销毁”。
///
/// 从前 `into_rx` 走的是 `Rx::new_signal(...)`，于是 `dispatch::is_disposed`
/// 拿一个 stored value 的句柄去查 `is_signal_valid` —— stored value 根本不在
/// `reactive` 表里，永远返回 false，句柄因此永远被报成已销毁。
#[test]
fn a_stored_value_turned_into_an_rx_is_not_reported_as_disposed() {
    use crate::traits::IntoRx;

    create_scope(|| {
        let sv = crate::reactivity::StoredValue::new(7i32);
        let rx = sv.into_rx();
        let (id, kind) = rx.inner.as_raw_parts().expect("有节点");

        assert!(
            !crate::reactivity::dispatch::is_disposed(id, kind),
            "刚建出来的 stored value 不该被报成已销毁"
        );
    });
}

#[test]
fn dispatcher_restores_the_handle_kind_before_reading() {
    fn increment(value: &i32) -> i32 {
        *value + 1
    }

    create_scope(|| {
        let signal_id = silex_reactivity::signal::create(7i32);
        let stored_id = silex_reactivity::store::create(9i32);
        let closure_id =
            silex_reactivity::store::create(Box::new(|| 11i32) as Box<dyn Fn() -> i32>);
        let op = StaticMapPayload::<i32>::new1(signal_id.raw(), increment, false);
        let op_rx: Rx<i32> = Rx::new_op(op);
        let (op_id, op_kind) = op_rx.inner.as_raw_parts().expect("op has an id");

        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(
                signal_id.raw(),
                RxNodeKind::Signal,
                |value| *value,
            ),
            Some(7)
        );
        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(
                signal_id.raw(),
                RxNodeKind::Stored,
                |value| *value,
            ),
            None
        );
        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(
                stored_id.raw(),
                RxNodeKind::Stored,
                |value| *value,
            ),
            Some(9)
        );
        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(
                stored_id.raw(),
                RxNodeKind::Signal,
                |value| *value,
            ),
            None
        );
        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(
                closure_id.raw(),
                RxNodeKind::Closure,
                |value| *value,
            ),
            Some(11)
        );
        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(op_id, op_kind, |value| *value,),
            Some(8)
        );
        assert_eq!(
            dispatch::rx_try_with_node_untracked::<i32, _>(op_id, RxNodeKind::Stored, |value| {
                *value
            },),
            None
        );
    });
}

#[test]
fn dispatcher_disposed_checks_follow_the_runtime_tag() {
    create_scope(|| {
        let signal_id = silex_reactivity::signal::create(1u8);
        let stored_id = silex_reactivity::store::create(2u8);
        let closure_id = silex_reactivity::store::create(Box::new(|| 3u8) as Box<dyn Fn() -> u8>);
        let op: Rx<u8> = Rx::new_op(StaticMapPayload::<u8>::new1(
            signal_id.raw(),
            |value: &u8| *value,
            false,
        ));
        let (op_id, _) = op.inner.as_raw_parts().expect("op has an id");

        assert!(!dispatch::is_disposed(signal_id.raw(), RxNodeKind::Signal));
        assert!(dispatch::is_disposed(signal_id.raw(), RxNodeKind::Stored));
        assert!(!dispatch::is_disposed(stored_id.raw(), RxNodeKind::Stored));
        assert!(dispatch::is_disposed(stored_id.raw(), RxNodeKind::Signal));
        assert!(!dispatch::is_disposed(
            closure_id.raw(),
            RxNodeKind::Closure
        ));
        assert!(dispatch::is_disposed(closure_id.raw(), RxNodeKind::Signal));
        assert!(!dispatch::is_disposed(op_id, RxNodeKind::Op));
        assert!(dispatch::is_disposed(op_id, RxNodeKind::Signal));
    });
}

#[test]
fn raw_op_payload_layout_is_stable() {
    assert_eq!(std::mem::size_of::<silex_reactivity::RawId>(), 8);
    assert_eq!(std::mem::align_of::<silex_reactivity::RawId>(), 4);
    assert_eq!(std::mem::size_of::<silex_reactivity::SignalId>(), 8);
    assert_eq!(std::mem::align_of::<silex_reactivity::SignalId>(), 4);
    assert_eq!(std::mem::size_of::<silex_reactivity::StoredId>(), 8);
    assert_eq!(std::mem::align_of::<silex_reactivity::StoredId>(), 4);
    assert_eq!(std::mem::size_of::<UnifiedStaticMapPayload<()>>(), 64);
    assert_eq!(std::mem::align_of::<UnifiedStaticMapPayload<()>>(), 8);
}

#[test]
fn tuple_n_mapper_reads_raw_metadata_without_reinterpreting_vec() {
    create_scope(|| {
        let first = RwSignal::new(1i32);
        let second = RwSignal::new(2i32);
        let third = RwSignal::new(3i32);
        let rx = (
            first.read_signal(),
            second.read_signal(),
            third.read_signal(),
        )
            .into_rx();

        assert_eq!(rx.get(), (1, 2, 3));
        first.set(4);
        assert_eq!(rx.get(), (4, 2, 3));
    });
}

#[test]
fn tuple_six_mapper_keeps_all_raw_inputs() {
    create_scope(|| {
        let first = RwSignal::new(1i32);
        let second = RwSignal::new(2i32);
        let third = RwSignal::new(3i32);
        let fourth = RwSignal::new(4i32);
        let fifth = RwSignal::new(5i32);
        let sixth = RwSignal::new(6i32);
        let rx = (
            first.read_signal(),
            second.read_signal(),
            third.read_signal(),
            fourth.read_signal(),
            fifth.read_signal(),
            sixth.read_signal(),
        )
            .into_rx();

        assert_eq!(rx.get(), (1, 2, 3, 4, 5, 6));
        sixth.set(7);
        assert_eq!(rx.get(), (1, 2, 3, 4, 5, 7));
    });
}
