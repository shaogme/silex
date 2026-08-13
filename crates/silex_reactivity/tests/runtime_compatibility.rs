use silex_reactivity::{ComputationInitError, ErrorHandler, Runtime, RuntimeInputs, Scope};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn parent_child_owned_and_root_scopes_accept_same_family_inputs() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let input = source.runtime_input();
            let owned = scope.owned_scope().expect("fallible reactive creation");
            let nested = owned.child().expect("child scope creation");
            assert!(
                scope
                    .effect_from(
                        RuntimeInputs::single(input.clone()),
                        || Ok(()),
                        handler(scope)
                    )
                    .is_ok()
            );
            assert!(
                owned
                    .effect_from(
                        RuntimeInputs::single(input.clone()),
                        || Ok(()),
                        handler(scope)
                    )
                    .is_ok()
            );
            assert!(
                nested
                    .effect_from(
                        RuntimeInputs::single(input.clone()),
                        || Ok(()),
                        handler(scope)
                    )
                    .is_ok()
            );

            scope
                .child(|child| {
                    assert!(
                        child
                            .effect_from(RuntimeInputs::single(input), || Ok(()), handler(child))
                            .is_ok()
                    );
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");

    let mut root_runtime = Runtime::new();
    let root = root_runtime.run().expect("runtime root creation");
    {
        let scope = root.scope();
        let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
        let input = source.runtime_input();
        assert!(
            scope
                .effect_from(RuntimeInputs::single(input), || Ok(()), handler(scope))
                .is_ok()
        );
    }
    assert!(root.is_active());
}

#[test]
fn different_schedulers_are_rejected_even_when_scope_ids_are_reused() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_input = first
        .child(|scope| {
            let (source, _) = scope.signal(1i32).expect("fallible reactive creation");
            source.runtime_input()
        })
        .expect("runtime child");
    let result = second
        .child(|scope| {
            scope
                .effect_from(
                    RuntimeInputs::single(foreign_input),
                    move || Ok(()),
                    handler(scope),
                )
                .map(|_| ())
        })
        .expect("runtime child");

    assert!(matches!(
        result,
        Err(ComputationInitError::Registration(
            silex_reactivity::ReactiveError::RuntimeMismatch,
        ))
    ));
}
