use silex_core::{
    ErrorHandler, Runtime, Scope, SilexError, SilexResult,
    traits::{RxBase, RxGet, RxRead},
};
use std::{cell::Cell, rc::Rc};

struct NonCloneValue;

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn tuple_tracking_accepts_non_cloneable_members() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("signal should initialize");
            let (second, set_second) = scope
                .signal(NonCloneValue)
                .expect("signal should initialize");
            let sources = (first, second);
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || -> SilexResult<()> {
                        sources.track()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("tuple effect should initialize");

            assert_eq!(runs.get(), 1);
            set_first.set(2).expect("signal should be writable");
            assert_eq!(runs.get(), 2);
            set_second
                .set(NonCloneValue)
                .expect("signal should be writable");
            assert_eq!(runs.get(), 3);
        })
        .expect("child scope should initialize");
}

#[test]
fn cloneable_tuple_supports_tracked_and_untracked_reads() -> SilexResult<()> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (first, set_first) = scope.signal(1_i32).expect("signal should initialize");
        let (second, _) = scope.signal(2_i32).expect("signal should initialize");
        let sources = (first, second);

        assert_eq!(sources.get()?, (1, 2));
        assert_eq!(sources.with(|value| value.0 + value.1)?, 3);
        assert_eq!(sources.with_untracked(|value| value.0 + value.1)?, 3);

        set_first.set(7).expect("signal should be writable");
        assert_eq!(sources.with(|value| value.0 + value.1)?, 9);
        Ok::<(), SilexError>(())
    })??;
    Ok(())
}
