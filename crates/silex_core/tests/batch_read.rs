use silex_core::{
    ErrorHandlerToken, ReadSignal, Runtime, Scope, SilexError, SilexResult, batch_read,
    batch_read_untracked,
};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandlerToken<'scope> {
    scope
        .error_handler(|_| {})
        .expect("error handler registration")
}

fn read_pair(first: &ReadSignal<'_, i32>, second: &ReadSignal<'_, i32>) -> SilexResult<i32> {
    batch_read!(first, second => |left: i32, right: i32| *left + *right).and_then(|result| result)
}

#[test]
fn batch_read_tracks_every_source_and_reads_values_in_order() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("first signal");
            let (second, set_second) = scope.signal(2_i32).expect("second signal");
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || {
                        let expected = 3 * (runs_in_effect.get() + 1);
                        assert_eq!(read_pair(&first, &second)?, expected);
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_first.set(4).expect("first signal should update");
            assert_eq!(runs.get(), 2);
            set_second.set(5).expect("second signal should update");
            assert_eq!(runs.get(), 3);
        })
        .expect("runtime child should initialize");
}

#[test]
fn batch_read_tracks_parent_sources_inside_a_child_callback() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("first signal");
            let (second, set_second) = scope.signal(2_i32).expect("second signal");
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || {
                        scope
                            .child(|_| {
                                let expected = 3 * (runs_in_effect.get() + 1);
                                assert_eq!(read_pair(&first, &second)?, expected);
                                Ok::<(), SilexError>(())
                            })
                            .and_then(|result| result)?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_first.set(4).expect("first signal should update");
            assert_eq!(runs.get(), 2);
            set_second.set(5).expect("second signal should update");
            assert_eq!(runs.get(), 3);
        })
        .expect("runtime child should initialize");
}

#[test]
fn batch_read_untracked_does_not_subscribe_its_sources() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("first signal");
            let (second, set_second) = scope.signal(2_i32).expect("second signal");
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || {
                        let sum = batch_read_untracked!(first, second =>
                            |left: i32, right: i32| *left + *right)
                        .and_then(|result| result)?;
                        assert_eq!(sum, 3);
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_first.set(4).expect("first signal should update");
            set_second.set(5).expect("second signal should update");
            assert_eq!(runs.get(), 1);
        })
        .expect("runtime child should initialize");
}

#[test]
fn batch_read_untracked_keeps_nested_reads_tracked() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (untracked_source, set_untracked_source) =
                scope.signal(1_i32).expect("untracked source");
            let (tracked_source, set_tracked_source) = scope.signal(2_i32).expect("tracked source");
            let runs_in_effect = runs.clone();
            let seen = Rc::new(Cell::new(0));
            let seen_in_effect = seen.clone();

            scope
                .effect(
                    move || {
                        batch_read_untracked!(untracked_source => |value: i32| {
                            seen_in_effect.set(*value);
                            tracked_source.get()?;
                            Ok::<(), SilexError>(())
                        })
                        .and_then(|result| result)?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            assert_eq!(seen.get(), 1);
            set_untracked_source
                .set(3)
                .expect("untracked source should update");
            assert_eq!(runs.get(), 1);
            assert_eq!(seen.get(), 1);
            set_tracked_source
                .set(4)
                .expect("tracked source should update");
            assert_eq!(runs.get(), 2);
            assert_eq!(seen.get(), 3);
        })
        .expect("runtime child should initialize");
}
