use silex_core::traits::RxOptionExt;
use silex_core::{
    ErrorHandler, PromotionPlan, ReactiveSource, Runtime, RuntimeInputs, RxData, RxValue, Scope,
    SilexError,
    logic::{Map, Memoize, ReactivePartialEq, ReactivePartialOrd},
    runtime_inputs_of,
};
use std::{cell::Cell, rc::Rc};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

struct DeclaredExternalSource {
    inputs: RuntimeInputs,
    materialized: Rc<Cell<bool>>,
}

impl RxValue for DeclaredExternalSource {
    type Value = i32;
}

impl<'scope> ReactiveSource<'scope> for DeclaredExternalSource {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, i32>
    where
        Self: Sized,
        i32: Sized + RxData + 'scope,
    {
        let materialized = self.materialized;
        PromotionPlan::derived(self.inputs, move |scope, inputs, error_handler| {
            materialized.set(true);
            Ok(scope
                .derived_from(inputs, || Ok(1i32), error_handler)
                .unwrap_or_else(|error| panic!("derived should initialize: {error}")))
        })
    }
}

#[test]
fn same_runtime_parent_child_promotion_and_propagation_are_valid() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(2i32).expect("signal should initialize");
            let promoted = scope
                .promote(source, handler(scope))
                .expect("promotion should initialize");
            let mapped = promoted
                .map(|value| value + 1, handler(scope))
                .expect("derived map should initialize");

            assert_eq!(mapped.get().expect("mapped value should be readable"), 3);
            set_source.set(7).expect("signal should be writable");
            assert_eq!(mapped.get().expect("mapped value should be readable"), 8);

            scope
                .child(|child| {
                    let (local, set_local) = child.signal(4i32).expect("signal should initialize");
                    let local = child
                        .promote(local, handler(child))
                        .expect("promotion should initialize");
                    let mut inputs = promoted.runtime_inputs();
                    inputs.extend(&local.runtime_inputs());
                    let derived = child
                        .derived_from(
                            inputs,
                            move || Ok(promoted.get()? + local.get()?),
                            handler(child),
                        )
                        .expect("derived should initialize");
                    assert_eq!(derived.get().expect("derived should be readable"), 11);
                    set_source.set(9).expect("signal should be writable");
                    assert_eq!(derived.get().expect("derived should be readable"), 13);
                    set_local.set(5).expect("signal should be writable");
                    assert_eq!(derived.get().expect("derived should be readable"), 14);
                })
                .expect("child scope should initialize");
        })
        .expect("child scope should initialize");
}

#[test]
fn foreign_inputs_are_rejected_before_target_derived_creation() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first
        .child(|scope| {
            let (source, _) = scope.signal(1i32).expect("signal should initialize");
            runtime_inputs_of(source)
        })
        .expect("child scope should initialize");
    let result = second
        .child(|scope| {
            scope
                .derived_from(foreign_inputs, || Ok(1i32), handler(scope))
                .map(|_| ())
        })
        .expect("child scope should initialize");

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(
            silex_core::ReactiveError::RuntimeMismatch,
        ))
    ));
}

#[test]
fn foreign_inputs_are_rejected_before_target_memo_creation() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first
        .child(|scope| {
            let (source, _) = scope.signal(1i32).expect("signal should initialize");
            runtime_inputs_of(source)
        })
        .expect("child scope should initialize");
    let called = Rc::new(Cell::new(false));
    let called_for_memo = called.clone();

    let result = second
        .child(|scope| {
            scope
                .memo_from(foreign_inputs, move |_| {
                    called_for_memo.set(true);
                    1i32
                })
                .map(|_| ())
        })
        .and_then(|result| result);

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(
            silex_core::ReactiveError::RuntimeMismatch,
        ))
    ));
    assert!(!called.get());
}

#[test]
fn external_promotion_plan_validates_inputs_before_materializing() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first
        .child(|scope| {
            let (source, _) = scope.signal(1i32).expect("signal should initialize");
            runtime_inputs_of(source)
        })
        .expect("child scope should initialize");
    let materialized = Rc::new(Cell::new(false));
    let materialized_for_source = materialized.clone();

    let result = second
        .child(|scope| {
            scope
                .promote(
                    DeclaredExternalSource {
                        inputs: foreign_inputs,
                        materialized: materialized_for_source,
                    },
                    handler(scope),
                )
                .map(|_| ())
        })
        .and_then(|result| result);

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(
            silex_core::ReactiveError::RuntimeMismatch
        ))
    ));
    assert!(!materialized.get());
}

#[test]
fn foreign_inputs_are_rejected_before_target_effect_creation() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first
        .child(|scope| {
            let (source, _) = scope.signal(1i32).expect("signal should initialize");
            runtime_inputs_of(source)
        })
        .expect("child scope should initialize");
    let called = Rc::new(Cell::new(false));
    let called_for_effect = called.clone();

    let result = second
        .child(|scope| {
            scope
                .effect_from(
                    foreign_inputs,
                    move || {
                        called_for_effect.set(true);
                        Ok(())
                    },
                    handler(scope),
                )
                .map(|_| ())
        })
        .and_then(|result| result);

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(
            silex_core::ReactiveError::RuntimeMismatch,
        ))
    ));
    assert!(!called.get());
}

#[test]
fn option_and_tuple_promotions_track_checked_sources() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1i32).expect("signal should initialize");
            let (second, set_second) = scope.signal(2i32).expect("signal should initialize");
            let tuple = scope
                .promote((first, second), handler(scope))
                .expect("tuple promotion should initialize");
            let (optional_source, set_optional) =
                scope.signal(Some(3i32)).expect("signal should initialize");
            let optional = scope
                .promote(optional_source, handler(scope))
                .expect("option promotion should initialize");
            let selected = optional
                .unwrap_or(scope, 0, handler(scope))
                .expect("option selection should initialize");

            assert_eq!(
                tuple
                    .with(|value| value.0 + value.1)
                    .expect("tuple should be readable"),
                3
            );
            assert_eq!(
                selected.get().expect("selected value should be readable"),
                3
            );
            set_first.set(4).expect("signal should be writable");
            set_second.set(5).expect("signal should be writable");
            assert_eq!(
                tuple
                    .with(|value| value.0 + value.1)
                    .expect("tuple should be readable"),
                9
            );
            set_optional
                .set(Some(6))
                .expect("signal should be writable");
            assert_eq!(
                selected.get().expect("selected value should be readable"),
                6
            );
        })
        .expect("child scope should initialize");
}

#[test]
fn operator_and_slice_promotions_use_the_target_scheduler() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (value, set_value) = scope.signal(3i32).expect("signal should initialize");
            let mapped = value
                .map(scope, |value| value * 2, handler(scope))
                .expect("map should initialize");
            let memo = value
                .memo(scope, handler(scope))
                .expect("memo should initialize");
            let added = scope
                .promote(value, handler(scope))
                .expect("promotion should initialize")
                .add(4, handler(scope))
                .expect("addition should initialize");
            let equal = value
                .equals(scope, 3, handler(scope))
                .expect("comparison should initialize");
            let positive = value
                .greater_than(scope, 0, handler(scope))
                .expect("comparison should initialize");
            let (pair, set_pair) = scope
                .signal((1i32, 2i32))
                .expect("signal should initialize");
            let first = scope
                .promote(pair.slice(|pair| &pair.0), handler(scope))
                .expect("slice promotion should initialize");

            assert_eq!(mapped.get().expect("mapped value should be readable"), 6);
            assert_eq!(memo.get().expect("memo should be readable"), 3);
            assert_eq!(added.get().expect("added value should be readable"), 7);
            assert!(equal.get().expect("comparison should be readable"));
            assert!(positive.get().expect("comparison should be readable"));
            assert_eq!(first.get().expect("slice should be readable"), 1);

            set_value.set(5).expect("signal should be writable");
            set_pair.set((8, 9)).expect("signal should be writable");
            assert_eq!(mapped.get().expect("mapped value should be readable"), 10);
            assert_eq!(memo.get().expect("memo should be readable"), 5);
            assert_eq!(added.get().expect("added value should be readable"), 9);
            assert!(!equal.get().expect("comparison should be readable"));
            assert_eq!(first.get().expect("slice should be readable"), 8);
        })
        .expect("child scope should initialize");
}
