#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    CallbackInvokeError, ComputationInitError, ErrorHandlerToken, OwnerAccess, ReactiveError,
    Runtime,
};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'scope, E: 'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, E> {
    scope
        .error_handler(|_| {})
        .expect("error handler registration")
}

#[test]
fn ordinary_reads_track_across_child_scopes() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let runs_in_effect = runs.clone();
            let child = scope.create_child().expect("owned scope creation");
            child
                .access()
                .effect(
                    move || {
                        source.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("effect creation");

            assert_eq!(runs.get(), 1);
            set_source.set(1).expect("source update");
            assert_eq!(runs.get(), 2);
            child.close().expect("child scope disposal");
        })
        .expect("runtime child");
}

#[test]
fn child_transient_reads_do_not_escape_the_child_callback() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        let child_result = scope
                            .with_transient(|child| {
                                let (local, _) = child.signal(1_i32)?;
                                local.get()?;
                                Ok::<(), ReactiveError>(())
                            })
                            .expect("child transient scope disposal");
                        child_result?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("effect creation");

            assert_eq!(runs.get(), 1);
        })
        .expect("runtime child");
}

#[test]
fn foreign_tracked_read_fails_before_dirty_source_evaluation() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    foreign_root.with_access(|foreign_scope| {
        let (source, set_source) = foreign_scope.signal(1_i32).expect("source creation");
        let runs = Rc::new(Cell::new(0));
        let runs_in_derived = runs.clone();
        let derived = foreign_scope
            .computed_always(
                move || {
                    runs_in_derived.set(runs_in_derived.get() + 1);
                    source.get().map(|value| value + 1)
                },
                handler(foreign_scope),
            )
            .expect("derived creation");
        assert_eq!(derived.get().expect("initial derived value"), 2);
        assert_eq!(runs.get(), 1);
        set_source.set(2).expect("foreign source update");

        let result = target_root.with_access(|target_scope| {
            target_scope
                .effect(
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
        assert_eq!(runs.get(), 1);
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}

#[test]
fn foreign_untracked_read_is_allowed_and_does_not_subscribe() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let runs = Rc::new(Cell::new(0));

    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");
    foreign_root.with_access(|foreign_scope| {
        let (foreign_source, set_foreign_source) =
            foreign_scope.signal(1_i32).expect("foreign source");
        target_root.with_access(|scope| {
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        assert_eq!(foreign_source.get_untracked()?, 1);
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("untracked effect creation");
        });

        assert_eq!(runs.get(), 1);
        set_foreign_source.set(2).expect("foreign source update");
        assert_eq!(runs.get(), 1);
    });
    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}
