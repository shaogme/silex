#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    CallbackInvokeError, ComputationInitError, EffectPhase, ErrorHandlerToken, OwnerAccess,
    ReactiveError, Runtime,
};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'scope, E: 'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, E> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn same_runtime_child_scope_reads_are_reactive() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(1_i32).expect("source signal");
            let child = scope.create_child().expect("owned scope");
            let runs_in_effect = runs.clone();
            child
                .access()
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_source.set(2).expect("source should update");
            assert_eq!(runs.get(), 2);
            child.close().expect("owned scope disposal");
        })
        .expect("runtime child should initialize");
}

#[test]
fn foreign_tracked_reads_are_rejected_before_source_evaluation() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    foreign_root.with_access(|foreign_scope| {
        let evaluations = Rc::new(Cell::new(0));
        let evaluations_in_derived = evaluations.clone();
        let (source, _) = foreign_scope.signal(1_i32).expect("foreign source");
        let derived = foreign_scope
            .computed_always(
                move || {
                    evaluations_in_derived.set(evaluations_in_derived.get() + 1);
                    source.get()
                },
                handler(foreign_scope),
            )
            .expect("foreign derived");
        assert_eq!(derived.get().expect("derived value"), 1);
        assert_eq!(evaluations.get(), 1);

        let result = target_root.with_access(|target_scope| {
            target_scope
                .effect(
                    EffectPhase::Normal,
                    move || derived.get().map(|_| ()),
                    handler::<CallbackInvokeError<ReactiveError>>(target_scope),
                )
                .map(|_| ())
        });

        assert!(matches!(
            result,
            Err(ComputationInitError::Initial(_))
                | Err(ComputationInitError::Registration(
                    ReactiveError::RuntimeMismatch
                ))
        ));
        assert_eq!(evaluations.get(), 1);
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[test]
fn foreign_untracked_reads_are_allowed_without_subscription() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");
    let runs = Rc::new(Cell::new(0));

    foreign_root.with_access(|foreign_scope| {
        let (source, set_source) = foreign_scope.signal(1_i32).expect("foreign source");
        target_root.with_access(|target_scope| {
            let runs_in_effect = runs.clone();
            target_scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get_untracked()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler::<ReactiveError>(target_scope),
                )
                .expect("effect should initialize");
        });

        assert_eq!(runs.get(), 1);
        set_source.set(2).expect("foreign source update");
        assert_eq!(runs.get(), 1);
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[test]
fn runtime_boundary_rejects_foreign_untracked_reads() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    foreign_root.with_access(|foreign_scope| {
        let (foreign_source, _) = foreign_scope.signal(1_i32).expect("foreign source");
        let result = target_root.with_access(|target_scope| {
            target_scope.with_runtime(|| foreign_source.get_untracked())
        });
        let nested_result = target_root.with_access(|target_scope| {
            target_scope.with_runtime(|| target_scope.untrack(|| foreign_source.get_untracked()))
        });

        assert!(matches!(result, Ok(Err(ReactiveError::RuntimeMismatch))));
        assert!(matches!(
            nested_result,
            Ok(Ok(Err(ReactiveError::RuntimeMismatch)))
        ));
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[test]
fn cleanup_untracked_reentry_can_build_another_runtime_binding() {
    let mut cleanup_runtime = Runtime::new();
    let cleanup_root = cleanup_runtime.owner().expect("cleanup root");
    let mut reentrant_runtime = Runtime::new();
    let reentrant_root = reentrant_runtime.owner().expect("reentrant root");
    let reentered = Rc::new(Cell::new(false));
    let reentered_in_cleanup = reentered.clone();

    cleanup_root.with_access(|scope| {
        scope
            .on_cleanup(
                move || {
                    reentrant_root.with_access(|reentrant_scope| {
                        let (source, _) = reentrant_scope.signal(1_i32)?;
                        source.get_untracked()?;
                        reentered_in_cleanup.set(true);
                        Ok(())
                    })
                },
                handler::<ReactiveError>(scope),
            )
            .expect("cleanup registration");
    });

    cleanup_root.close().expect("cleanup root disposal");
    assert!(reentered.get());
}

#[test]
fn untrack_only_masks_the_runtime_that_owns_the_scope() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");
    let runs = Rc::new(Cell::new(0));

    foreign_root.with_access(|foreign_scope| {
        target_root.with_access(|target_scope| {
            let (source, set_source) = target_scope.signal(1_i32).expect("target source");
            let runs_in_effect = runs.clone();
            target_scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        foreign_scope.untrack(|| source.get())??;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler::<ReactiveError>(target_scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_source.set(2).expect("target source update");
            assert_eq!(runs.get(), 2);
        });
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}
