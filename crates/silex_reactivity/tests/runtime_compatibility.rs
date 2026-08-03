use silex_reactivity::{Runtime, RuntimeInputs};

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
                .try_effect_from(RuntimeInputs::single(input.clone()), || {})
                .is_ok()
        );
        assert!(
            owned
                .try_effect_from(RuntimeInputs::single(input.clone()), || {})
                .is_ok()
        );
        assert!(
            nested
                .try_effect_from(RuntimeInputs::single(input.clone()), || {})
                .is_ok()
        );

        scope.child(|child| {
            assert!(
                child
                    .try_effect_from(RuntimeInputs::single(input), || {})
                    .is_ok()
            );
        });
    });

    let mut root_runtime = Runtime::new();
    let root = root_runtime.run(|scope| {
        let (source, _) = scope.signal(0i32);
        let input = source.runtime_input();
        assert!(
            scope
                .try_effect_from(RuntimeInputs::single(input), || {})
                .is_ok()
        );
    });
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
            .try_effect_from(RuntimeInputs::single(foreign_input), move || {})
            .map(|_| ())
    });

    assert!(matches!(
        result,
        Err(silex_reactivity::ReactiveError::RuntimeMismatch)
    ));
}
