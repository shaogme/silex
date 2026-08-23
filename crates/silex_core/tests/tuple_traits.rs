use silex_core::{
    EffectPhase, ErrorHandlerToken, OwnerAccess, Runtime, RxBase, RxGet, RxReadTuple2, SilexResult,
};
use std::cell::Cell;
use std::rc::Rc;

struct NonClone(u32);

fn handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn tuple_get_tracks_each_member() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let first = owner.signal(1_i32).expect("first signal");
            let second = owner.signal(2_i32).expect("second signal");
            let sources = (first, second);
            let runs_in_effect = runs.clone();

            owner
                .effect(
                    EffectPhase::Normal,
                    move || -> SilexResult<()> {
                        let (first_value, second_value) = sources.get()?;
                        assert_eq!(first_value + second_value, 3 + runs_in_effect.get() * 2);
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("tuple effect should initialize");

            assert_eq!(runs.get(), 1);
            first.set(3).expect("first signal should update");
            assert_eq!(runs.get(), 2);
            second.set(4).expect("second signal should update");
            assert_eq!(runs.get(), 3);
        })
        .expect("runtime child should initialize");
}

#[test]
fn tuple_with_reads_returns_an_owned_value() {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let first = owner.signal(4_i32).expect("first signal");
            let second = owner.signal(5_i32).expect("second signal");
            let sources = (first, second);
            let value = sources
                .with(|(first_value, second_value)| (*first_value, *second_value))
                .expect("tuple read should succeed");

            assert_eq!(value, (4, 5));
        })
        .expect("runtime child should initialize");
}

#[test]
fn tuple_untracked_get_does_not_subscribe() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let first = owner.signal(1_i32).expect("first signal");
            let second = owner.signal(2_i32).expect("second signal");
            let sources = (first, second);
            let runs_in_effect = runs.clone();

            owner
                .effect(
                    EffectPhase::Normal,
                    move || -> SilexResult<()> {
                        assert_eq!(sources.get_untracked()?, (1, 2));
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("tuple effect should initialize");

            assert_eq!(runs.get(), 1);
            first.set(3).expect("first signal should update");
            assert_eq!(runs.get(), 1);
        })
        .expect("runtime child should initialize");
}

#[test]
fn base_track_accepts_non_clone_sources() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let source = owner.signal(NonClone(1)).expect("source signal");
            let sources = (source, source);
            sources.track().expect("tuple tracking should succeed");

            let runs_in_effect = runs.clone();
            owner
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.track()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("tracking effect should initialize");

            assert_eq!(runs.get(), 1);
            source
                .update(|value| value.0 += 1)
                .expect("source should update");
            assert_eq!(runs.get(), 2);
        })
        .expect("runtime child should initialize");
}
