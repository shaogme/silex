use silex_core::traits::RxOptionExt;
use silex_core::{
    Runtime,
    logic::{Map, Memoize, ReactivePartialEq, ReactivePartialOrd},
};
use std::{cell::Cell, rc::Rc};

#[test]
fn same_runtime_parent_child_promotion_and_propagation_are_valid() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, set_source) = scope.signal(2i32);
        let promoted = scope.promote(source);
        let mapped = promoted.map(|value| value + 1);

        assert_eq!(mapped.get(), 3);
        set_source.set(7);
        assert_eq!(mapped.get(), 8);

        scope.child(|child| {
            let (local, set_local) = child.signal(4i32);
            let local = child.promote(local);
            let mut inputs = promoted.runtime_inputs();
            inputs.extend(&local.runtime_inputs());
            let derived = child.derived_from(inputs, move || promoted.get() + local.get());
            assert_eq!(derived.get(), 11);
            set_source.set(9);
            assert_eq!(derived.get(), 13);
            set_local.set(5);
            assert_eq!(derived.get(), 14);
        });
    });
}

#[test]
fn foreign_inputs_are_rejected_before_target_derived_creation() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first.child(|scope| {
        let (source, _) = scope.signal(1i32);
        scope.promote(source).runtime_inputs()
    });
    let result = second.child(|scope| scope.try_derived_from(foreign_inputs, || 1i32).map(|_| ()));

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(message))
            if message.contains("不同")
    ));
}

#[test]
fn foreign_inputs_are_rejected_before_target_effect_creation() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first.child(|scope| {
        let (source, _) = scope.signal(1i32);
        scope.promote(source).runtime_inputs()
    });
    let called = Rc::new(Cell::new(false));
    let called_for_effect = called.clone();

    let result = second.child(|scope| {
        scope
            .try_effect_from(foreign_inputs, move || {
                called_for_effect.set(true);
            })
            .map(|_| ())
    });

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(message))
            if message.contains("不同")
    ));
    assert!(!called.get());
}

#[test]
fn option_and_tuple_promotions_track_checked_sources() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (first, set_first) = scope.signal(1i32);
        let (second, set_second) = scope.signal(2i32);
        let tuple = scope.promote((first, second));
        let (optional_source, set_optional) = scope.signal(Some(3i32));
        let optional = scope.promote(optional_source);
        let selected = optional.unwrap_or(&scope, 0);

        assert_eq!(tuple.with(|value| value.0 + value.1), 3);
        assert_eq!(selected.get(), 3);
        set_first.set(4);
        set_second.set(5);
        assert_eq!(tuple.with(|value| value.0 + value.1), 9);
        set_optional.set(Some(6));
        assert_eq!(selected.get(), 6);
    });
}

#[test]
fn operator_and_slice_promotions_use_the_target_scheduler() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(3i32);
        let mapped = value.map(&scope, |value| value * 2);
        let memo = value.memo(&scope);
        let added = scope.promote(value) + 4;
        let equal = value.equals(&scope, 3);
        let positive = value.greater_than(&scope, 0);
        let (pair, set_pair) = scope.signal((1i32, 2i32));
        let first = scope.promote(pair.slice(|pair| &pair.0));

        assert_eq!(mapped.get(), 6);
        assert_eq!(memo.get(), 3);
        assert_eq!(added.get(), 7);
        assert!(equal.get());
        assert!(positive.get());
        assert_eq!(first.get(), 1);

        set_value.set(5);
        set_pair.set((8, 9));
        assert_eq!(mapped.get(), 10);
        assert_eq!(memo.get(), 5);
        assert_eq!(added.get(), 9);
        assert!(!equal.get());
        assert_eq!(first.get(), 8);
    });
}
