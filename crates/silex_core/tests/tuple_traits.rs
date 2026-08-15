use silex_core::{ErrorHandlerToken, Runtime, RxGet, RxRead, Scope, SilexResult};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandlerToken<'scope> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn tuple_get_tracks_each_member() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("first signal");
            let (second, set_second) = scope.signal(2_i32).expect("second signal");
            let sources = (first, second);
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || -> SilexResult<()> {
                        let (first_value, second_value) = sources.get()?;
                        assert_eq!(first_value + second_value, 3 + runs_in_effect.get() * 2);
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("tuple effect should initialize");

            assert_eq!(runs.get(), 1);
            set_first.set(3).expect("first signal should update");
            assert_eq!(runs.get(), 2);
            set_second.set(4).expect("second signal should update");
            assert_eq!(runs.get(), 3);
        })
        .expect("runtime child should initialize");
}

#[test]
fn tuple_with_reads_a_cloneable_snapshot() {
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| {
            let (first, _) = scope.signal(4_i32).expect("first signal");
            let (second, _) = scope.signal(5_i32).expect("second signal");
            let sources = (first, second);
            let snapshot = sources
                .with(|(first_value, second_value)| (*first_value, *second_value))
                .expect("tuple read should succeed");

            assert_eq!(snapshot, (4, 5));
        })
        .expect("runtime child should initialize");
}

#[test]
fn tuple_untracked_get_does_not_subscribe() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("first signal");
            let (second, _) = scope.signal(2_i32).expect("second signal");
            let sources = (first, second);
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || -> SilexResult<()> {
                        assert_eq!(sources.get_untracked()?, (1, 2));
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("tuple effect should initialize");

            assert_eq!(runs.get(), 1);
            set_first.set(3).expect("first signal should update");
            assert_eq!(runs.get(), 1);
        })
        .expect("runtime child should initialize");
}
