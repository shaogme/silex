use silex_reactivity::{
    ComputationInitError, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime,
};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'scope, E: 'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, E> {
    scope
        .error_handler(|_| {})
        .expect("error handler registration")
}

#[test]
fn detached_effect_survives_parent_reruns_and_stops_explicitly() {
    let mut runtime = Runtime::new();
    let parent_runs = Rc::new(Cell::new(0));
    let detached_runs = Rc::new(Cell::new(0));
    let detached_cleanups = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (parent_source, set_parent_source) =
                scope.signal(0_i32).expect("parent source creation");
            let (detached_source, set_detached_source) =
                scope.signal(0_i32).expect("detached source creation");
            let created = Rc::new(Cell::new(false));
            let parent_runs_in_effect = parent_runs.clone();
            let detached_runs_in_effect = detached_runs.clone();
            let detached_cleanups_in_effect = detached_cleanups.clone();
            let scope_in_effect = scope;
            scope
                .effect(
                    move || {
                        parent_source.get()?;
                        parent_runs_in_effect.set(parent_runs_in_effect.get() + 1);
                        if !created.replace(true) {
                            let detached_runs = detached_runs_in_effect.clone();
                            let detached_cleanups = detached_cleanups_in_effect.clone();
                            let scope_for_detached = scope_in_effect;
                            scope_in_effect
                                .effect_detached(
                                    move || {
                                        detached_source.get()?;
                                        detached_runs.set(detached_runs.get() + 1);
                                        let cleanup_count = detached_cleanups.clone();
                                        scope_for_detached.on_cleanup(
                                            move || {
                                                cleanup_count.set(cleanup_count.get() + 1);
                                                Ok(())
                                            },
                                            handler::<()>(scope_for_detached),
                                        )?;
                                        Ok(())
                                    },
                                    handler::<ReactiveError>(scope_for_detached),
                                )
                                .map_err(|_| ReactiveError::InvariantViolation)?;
                        }
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("parent effect should initialize");

            assert_eq!(parent_runs.get(), 1);
            assert_eq!(detached_runs.get(), 1);

            set_parent_source
                .set(1)
                .expect("parent source update should succeed");
            assert_eq!(parent_runs.get(), 2);
            assert_eq!(detached_runs.get(), 1);
            assert_eq!(detached_cleanups.get(), 0);

            set_detached_source
                .set(1)
                .expect("detached source update should succeed");
            assert_eq!(detached_runs.get(), 2);
            assert_eq!(detached_cleanups.get(), 1);
        })
        .expect("runtime operation should succeed");

    assert_eq!(detached_cleanups.get(), 2);
}

#[test]
fn detached_effect_stop_runs_cleanup_once_and_invalidates_the_handle() {
    let mut runtime = Runtime::new();
    let cleanups = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let cleanups_in_effect = cleanups.clone();
            let effect = scope
                .effect_detached(
                    move || {
                        let cleanups = cleanups_in_effect.clone();
                        scope.on_cleanup(
                            move || {
                                cleanups.set(cleanups.get() + 1);
                                Ok(())
                            },
                            handler::<()>(scope),
                        )?;
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("detached effect should initialize");

            assert!(effect.stop().expect("detached effect should stop"));
            assert_eq!(cleanups.get(), 1);
            assert!(!effect.stop().expect("stopped effect should be inert"));
        })
        .expect("runtime operation should succeed");

    assert_eq!(cleanups.get(), 1);
}

#[test]
fn ordinary_nested_effect_remains_a_child_of_the_parent_effect() {
    let mut runtime = Runtime::new();
    let child_cleanups = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let child_cleanups_in_effect = child_cleanups.clone();
            let scope_in_effect = scope;
            scope
                .effect(
                    move || {
                        source.get()?;
                        let child_cleanups = child_cleanups_in_effect.clone();
                        scope_in_effect
                            .effect(
                                move || {
                                    let cleanups = child_cleanups.clone();
                                    scope_in_effect.on_cleanup(
                                        move || {
                                            cleanups.set(cleanups.get() + 1);
                                            Ok(())
                                        },
                                        handler::<()>(scope_in_effect),
                                    )?;
                                    Ok(())
                                },
                                handler::<ReactiveError>(scope_in_effect),
                            )
                            .expect("nested effect should initialize");
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("parent effect should initialize");

            set_source.set(1).expect("source update should succeed");
            assert_eq!(child_cleanups.get(), 1);
        })
        .expect("runtime operation should succeed");
}

#[test]
fn failed_detached_effect_does_not_remain_reactive() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let cleanups = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let runs_in_effect = runs.clone();
            let cleanups_in_effect = cleanups.clone();
            let result = scope.effect_detached(
                move || {
                    source.get()?;
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    let cleanup_count = cleanups_in_effect.clone();
                    scope.on_cleanup(
                        move || {
                            cleanup_count.set(cleanup_count.get() + 1);
                            Ok(())
                        },
                        handler::<()>(scope),
                    )?;
                    Err(ReactiveError::InvariantViolation)
                },
                scope
                    .error_handler(|_: ReactiveError| {})
                    .expect("error handler registration"),
            );
            assert!(matches!(result, Err(ComputationInitError::Initial(_))));
            assert_eq!(runs.get(), 1);
            assert_eq!(cleanups.get(), 1);

            set_source.set(1).expect("source update should succeed");
            assert_eq!(runs.get(), 1);
            assert_eq!(cleanups.get(), 1);
        })
        .expect("runtime operation should succeed");
}
