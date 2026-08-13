use silex_reactivity::{
    CallbackInvokeError, ComputationInitError, ErrorHandler, ReactiveError, Runtime, Scope,
};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'scope, E: 'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, E> {
    scope
        .error_handler(|_| {})
        .expect("error handler registration")
}

#[test]
fn ordinary_reads_track_across_child_scopes() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let runs_in_effect = runs.clone();
            let child = scope.owned_scope().expect("owned scope creation");
            child
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
            child.dispose().expect("child scope disposal");
        })
        .expect("runtime child");
}

#[test]
fn child_transient_reads_do_not_escape_the_child_callback() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        scope
                            .child(|child| {
                                let (local, _) = child.signal(1_i32)?;
                                local.get()?;
                                Ok::<(), ReactiveError>(())
                            })
                            .and_then(|result| result)?;
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
    let foreign_root = foreign_runtime.run().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run().expect("target root");

    foreign_root.with_scope(|foreign_scope| {
        let (source, set_source) = foreign_scope.signal(1_i32).expect("source creation");
        let runs = Rc::new(Cell::new(0));
        let runs_in_derived = runs.clone();
        let derived = foreign_scope
            .derived(
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

        let result = target_root.with_scope(|target_scope| {
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

    target_root.dispose().expect("target root disposal");
    foreign_root.dispose().expect("foreign root disposal");
}

#[test]
fn foreign_untracked_read_is_allowed_and_does_not_subscribe() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.run().expect("foreign root");
    let runs = Rc::new(Cell::new(0));

    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run().expect("target root");
    foreign_root.with_scope(|foreign_scope| {
        let (foreign_source, set_foreign_source) =
            foreign_scope.signal(1_i32).expect("foreign source");
        target_root.with_scope(|scope| {
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
    target_root.dispose().expect("target root disposal");
    foreign_root.dispose().expect("foreign root disposal");
}
