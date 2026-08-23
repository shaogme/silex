#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{EffectPhase, ErrorHandlerToken, OwnerAccess, Runtime};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn test_any_value_soo_boundary_and_downcast() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            // 小于等于 24 字节类型 (SOO 内联路径)
            let s_i32 = scope.signal(42i32).expect("fallible reactive creation");
            assert_eq!(s_i32.get(), Ok(42));

            s_i32.set(100).expect("test operation should succeed");
            assert_eq!(s_i32.get(), Ok(100));

            // 刚好 24 字节类型 ([u8; 24])
            let arr24 = [7u8; 24];
            let s_24 = scope.signal(arr24).expect("fallible reactive creation");
            assert_eq!(s_24.get(), Ok([7u8; 24]));

            // 超过 24 字节类型 ([u8; 32]) (堆分配路径)
            let arr32 = [9u8; 32];
            let s_32 = scope.signal(arr32).expect("fallible reactive creation");
            assert_eq!(s_32.get(), Ok([9u8; 32]));

            s_32.update(|arr| arr[31] = 99)
                .expect("test operation should succeed");
            assert_eq!(s_32.get().expect("reactive read")[31], 99);
        })
        .expect("test operation should succeed");
}

#[test]
fn test_any_value_drop_semantics_on_stack_and_heap() {
    struct DropTracker(Rc<Cell<usize>>);
    impl Drop for DropTracker {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[allow(dead_code)]
    struct BigDropTracker([u8; 32], Rc<Cell<usize>>);

    impl Drop for BigDropTracker {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }

    let drop_counter_stack = Rc::new(Cell::new(0));
    let drop_counter_heap = Rc::new(Cell::new(0));

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let _s_stack = scope
                .signal(DropTracker(drop_counter_stack.clone()))
                .expect("fallible reactive creation");
            let _s_heap = scope.signal(BigDropTracker([0; 32], drop_counter_heap.clone()));

            assert_eq!(drop_counter_stack.get(), 0);
            assert_eq!(drop_counter_heap.get(), 0);
        })
        .expect("test operation should succeed");

    // 作用域销毁后，所有节点及其 Payload/AnyValue 应该被正确 Drop
    assert_eq!(drop_counter_stack.get(), 1);
    assert_eq!(drop_counter_heap.get(), 1);
}

#[test]
fn test_any_value_interior_mutability_inline() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let s_refcell = scope
                .signal(RefCell::new(Vec::new()))
                .expect("fallible reactive creation");
            let _ = s_refcell;
            s_refcell
                .read_signal()
                .with(|v| v.borrow_mut().push(10))
                .expect("test operation should succeed");
            s_refcell
                .read_signal()
                .with(|v| v.borrow_mut().push(20))
                .expect("test operation should succeed");

            assert_eq!(
                s_refcell.read_signal().with(|v| v.borrow().clone()),
                Ok(vec![10, 20])
            );
        })
        .expect("test operation should succeed");
}

#[test]
fn test_any_value_memo_skip_equal_update() {
    let memo_eval_count = Rc::new(Cell::new(0));
    let memo_eval_count_cloned = memo_eval_count.clone();

    let effect_eval_count = Rc::new(Cell::new(0));
    let effect_eval_count_cloned = effect_eval_count.clone();

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let sig = scope.signal(10i32).expect("fallible reactive creation");
            let memo = scope
                .computed(
                    move || {
                        memo_eval_count_cloned.set(memo_eval_count_cloned.get() + 1);
                        Ok(sig.get().expect("reactive read") * 2)
                    },
                    handler(scope),
                )
                .expect("memo creation");

            let memo_for_effect = memo;
            let _effect = scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        memo_for_effect.get().expect("reactive read");
                        effect_eval_count_cloned.set(effect_eval_count_cloned.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(memo.get(), Ok(20));
            assert_eq!(memo_eval_count.get(), 1);
            assert_eq!(effect_eval_count.get(), 1);

            // 设置相同的值
            sig.set(10).expect("test operation should succeed");

            // memo 计算虽重新求值比对，但由于 try_eq 相等，其更新版本不变，下游 effect 绝对不触发！
            assert_eq!(memo.get(), Ok(20));
            assert_eq!(memo_eval_count.get(), 2);
            assert_eq!(effect_eval_count.get(), 1); // 成功拦截下游 EffectHandle 触发！

            // 设置新值 15
            sig.set(15).expect("test operation should succeed");
            assert_eq!(memo.get(), Ok(30));
            assert_eq!(memo_eval_count.get(), 3);
            assert_eq!(effect_eval_count.get(), 2);
        })
        .expect("test operation should succeed");
}
