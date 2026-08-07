use silex_reactivity::{EffectInitError, ErrorHandler, Runtime, RuntimeInputs};

fn handler<'scope>() -> ErrorHandler<'scope, ()> {
    ErrorHandler::new(|_| {})
}

#[test]
fn parent_child_owned_and_root_scopes_accept_same_family_inputs() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, _) = scope.signal(0i32);
        let input = source.runtime_input();
        let owned = scope.owned_scope();
        let nested = owned.child();
        assert!(
            scope
                .effect_from(RuntimeInputs::single(input.clone()), || Ok(()), handler())
                .is_ok()
        );
        assert!(
            owned
                .effect_from(RuntimeInputs::single(input.clone()), || Ok(()), handler())
                .is_ok()
        );
        assert!(
            nested
                .effect_from(RuntimeInputs::single(input.clone()), || Ok(()), handler())
                .is_ok()
        );

        scope.child(|child| {
            assert!(
                child
                    .effect_from(RuntimeInputs::single(input), || Ok(()), handler())
                    .is_ok()
            );
        });
    });

    let mut root_runtime = Runtime::new();
    let root = root_runtime.run();
    {
        let scope = root.scope();
        let (source, _) = scope.signal(0i32);
        let input = source.runtime_input();
        assert!(
            scope
                .effect_from(RuntimeInputs::single(input), || Ok(()), handler())
                .is_ok()
        );
    }
    assert!(root.is_active());
}

#[test]
fn different_schedulers_are_rejected_even_when_scope_ids_are_reused() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_input = first.child(|scope| {
        let (source, _) = scope.signal(1i32);
        source.runtime_input()
    });
    let result = second.child(|scope| {
        scope
            .effect_from(
                RuntimeInputs::single(foreign_input),
                move || Ok(()),
                handler(),
            )
            .map(|_| ())
    });

    assert!(matches!(
        result,
        Err(EffectInitError::Registration(
            silex_reactivity::ReactiveError::RuntimeMismatch,
        ))
    ));
}
