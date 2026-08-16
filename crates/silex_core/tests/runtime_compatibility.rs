use silex_core::{
    ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime, SilexError, SilexErrorKind,
};
use std::cell::Cell;
use std::rc::Rc;

fn handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn same_runtime_child_scope_reads_are_reactive() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|owner| {
            let (source, set_source) = owner.signal(1_i32).expect("source signal");
            let child = owner.create_child().expect("owned owner");
            let child_owner = child.access();
            let runs_in_effect = runs.clone();
            child_owner
                .effect(
                    move || {
                        source.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(child_owner),
                )
                .expect("effect should initialize");

            set_source.set(2).expect("source should update");
            assert_eq!(runs.get(), 2);
        })
        .expect("runtime child should initialize");
}

#[test]
fn foreign_tracked_reads_are_rejected() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("foreign root");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.owner().expect("target root");

    foreign_root.with_access(|foreign_scope| {
        let (source, _) = foreign_scope.signal(1_i32).expect("foreign source");
        let result = target_root.with_access(|target_scope| {
            target_scope
                .effect(move || source.get().map(|_| ()), handler(target_scope))
                .map(|_| ())
        });
        assert!(matches!(
            result,
            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                ReactiveError::RuntimeMismatch,
            )))
        ));
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
                    move || {
                        source.get_untracked()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(target_scope),
                )
                .expect("effect should initialize");
        });

        set_source.set(2).expect("foreign source should update");
        assert_eq!(runs.get(), 1);
    });

    target_root.close().expect("target root disposal");
    foreign_root.close().expect("foreign root disposal");
}
