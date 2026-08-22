#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    CompletionSender, EffectPhase, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime,
    unwind_safe,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn owned_scope_keeps_effects_until_explicit_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    {
        let scope = root.access();
        let (read, write) = scope.signal(1i32).expect("fallible reactive creation");
        let runs = Rc::new(Cell::new(0));
        let cleanups = Rc::new(Cell::new(0));
        let owner = scope.create_child().expect("fallible reactive creation");

        let runs_for_effect = runs.clone();
        let _effect = owner
            .access()
            .effect(
                EffectPhase::Normal,
                move || {
                    read.with(|value| {
                        assert!(*value >= 1);
                    })
                    .expect("test operation should succeed");
                    runs_for_effect.set(runs_for_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");
        let cleanups_for_owner = cleanups.clone();
        owner
            .access()
            .on_cleanup(
                move || {
                    cleanups_for_owner.set(cleanups_for_owner.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

        assert_eq!(runs.get(), 1);
        write.set(2).expect("signal update");
        assert_eq!(runs.get(), 2);

        owner.close().expect("owner disposal");
        assert!(!owner.is_active().expect("owner active state"));
        assert_eq!(cleanups.get(), 1);
        write.set(3).expect("signal update");
        assert_eq!(runs.get(), 2);
        owner.close().expect("owner disposal");
        assert_eq!(cleanups.get(), 1);
    }

    root.close().expect("root disposal should succeed");
}

#[test]
fn detached_completion_survives_effect_disposal() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let endpoint = Rc::new(RefCell::new(None::<CompletionSender<(), ReactiveError>>));
    let hits = Rc::new(Cell::new(0));
    let access = root.access();
    let endpoint_for_effect = endpoint.clone();
    let hits_for_effect = hits.clone();
    let effect_access = access;
    let error_handler = access
        .error_handler(|_: ReactiveError| {})
        .expect("handler registration");
    let effect = access
        .effect(
            EffectPhase::Normal,
            move || {
                let hits = hits_for_effect.clone();
                let sender = effect_access.completion_sender_detached(unwind_safe(move |()| {
                    hits.set(hits.get() + 1);
                    Ok(())
                }))?;
                endpoint_for_effect.borrow_mut().replace(sender);
                Ok(())
            },
            error_handler,
        )
        .expect("effect creation");

    effect.stop().expect("effect disposal");
    let sender = endpoint
        .borrow_mut()
        .take()
        .expect("detached endpoint should be retained");
    assert_eq!(sender.submit(()), Ok(true));
    assert_eq!(hits.get(), 1);
    sender.cancel().expect("detached endpoint cancellation");
    root.close().expect("runtime root disposal");
}

#[test]
fn closing_an_owner_closes_nested_children_before_releasing_the_parent() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let child = root.access().create_child().expect("child creation");
    let grandchild = child.create_child().expect("grandchild creation");
    let cleanups = Rc::new(RefCell::new(Vec::new()));

    let grandchild_cleanups = cleanups.clone();
    grandchild
        .access()
        .on_cleanup(
            move || {
                grandchild_cleanups.borrow_mut().push("grandchild");
                Ok(())
            },
            handler(root.access()),
        )
        .expect("grandchild cleanup registration");
    let child_cleanups = cleanups.clone();
    child
        .access()
        .on_cleanup(
            move || {
                child_cleanups.borrow_mut().push("child");
                Ok(())
            },
            handler(root.access()),
        )
        .expect("child cleanup registration");

    root.close().expect("root close should be child-first");
    assert!(!root.is_active().expect("root active state"));
    assert!(!child.is_active().expect("child active state"));
    assert!(!grandchild.is_active().expect("grandchild active state"));
    assert_eq!(cleanups.borrow().as_slice(), &["grandchild", "child"]);
}

#[test]
fn owned_scope_cleanup_can_release_captured_stored_value() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let stored = scope.stored(1_i32).expect("fallible reactive creation");
            let owner = scope.create_child().expect("fallible reactive creation");
            let observed_in_cleanup = observed.clone();
            owner
                .access()
                .on_cleanup(
                    move || {
                        observed_in_cleanup.set(
                            stored
                                .update(|value| {
                                    *value = 2;
                                    *value
                                })
                                .expect("captured stored value should be available"),
                        );
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("owner cleanup should register");

            owner.close().expect("owner disposal");
            assert!(!owner.is_active().expect("owner active state"));
        })
        .expect("test operation should succeed");

    assert_eq!(observed.get(), 2);
}

#[test]
fn lexical_owned_scope_supports_borrowed_callbacks_and_nested_dispose() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let text = String::from("borrowed");
            let (read, write) = scope.signal(1i32).expect("fallible reactive creation");
            let owner = scope.create_child().expect("fallible reactive creation");
            let runs = Rc::new(Cell::new(0));
            let cleanups = Rc::new(Cell::new(0));

            let runs_for_effect = runs.clone();
            owner
                .access()
                .effect(
                    EffectPhase::Normal,
                    move || {
                        read.with(|value| {
                            assert!(*value >= 1);
                            assert_eq!(text.as_str(), "borrowed");
                        })
                        .expect("test operation should succeed");
                        runs_for_effect.set(runs_for_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            let child = owner.create_child().expect("child scope creation");
            let child_cleanups = cleanups.clone();
            child
                .access()
                .on_cleanup(
                    move || {
                        child_cleanups.set(child_cleanups.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");

            write.set(2).expect("signal update");
            assert_eq!(runs.get(), 2);
            child.close().expect("child disposal");
            owner.close().expect("owner disposal");
            assert_eq!(cleanups.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn owned_scope_completion_can_capture_scope_local_data() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let owner = scope.create_child().expect("fallible reactive creation");
            let local = String::from("owned");
            let seen_in_callback = seen.clone();
            let token = owner
                .access()
                .completion_once(unwind_safe(move |value: i32| {
                    assert_eq!(local, "owned");
                    seen_in_callback.set(value);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            assert!(token.submit(9).expect("completion submit"));
            owner.close().expect("owner disposal");
            assert!(!token.submit(10).expect("stale completion submit"));
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 9);
}

#[test]
fn fallible_owner_registration_rejects_inactive_scope() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let scope_for_cleanup = scope;
            let cleanup_error_handler = handler(scope);
            scope
                .on_cleanup(
                    move || {
                        assert_eq!(
                            scope_for_cleanup.on_cleanup(|| Ok(()), cleanup_error_handler),
                            Err(ReactiveError::NoSuchNode)
                        );
                        assert!(matches!(
                            scope_for_cleanup.create_child(),
                            Err(ReactiveError::NoSuchNode)
                        ));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    let mut root_runtime = Runtime::new();
    let root = root_runtime.owner().expect("runtime root creation");
    let root_scope = root.access();
    let owner = root_scope.create_child().expect("owner is active");
    assert!(
        owner
            .access()
            .on_cleanup(|| Ok(()), handler(root_scope))
            .is_ok()
    );
    owner.close().expect("owner disposal");
    assert_eq!(
        owner.access().on_cleanup(|| Ok(()), handler(root_scope)),
        Err(ReactiveError::NoSuchNode)
    );
    assert!(matches!(
        owner.create_child(),
        Err(ReactiveError::NoSuchNode)
    ));
    drop(owner);
    root.close().expect("root cleanup should succeed");
}

#[test]
fn fallible_cleanup_preserves_registration_order_during_dispose() {
    let mut runtime = Runtime::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let events_for_cleanup = events.clone();

    runtime
        .with_transient(|scope| {
            let scope_for_cleanup = scope;
            let cleanup_error_handler = handler(scope);
            scope
                .on_cleanup(
                    move || {
                        events_for_cleanup.borrow_mut().push("first");
                        assert_eq!(
                            scope_for_cleanup.on_cleanup(|| Ok(()), cleanup_error_handler),
                            Err(ReactiveError::NoSuchNode)
                        );
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert_eq!(events.borrow().as_slice(), ["first"]);
}

#[test]
fn persistent_child_adapter_preserves_topology_and_parent_close_is_idempotent() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let root_access = root.access();
    let branch = root_access
        .create_owned_child()
        .expect("persistent branch creation");
    let nested = branch
        .access()
        .create_owned_child()
        .expect("nested branch creation");
    let cleanup_order = Rc::new(RefCell::new(Vec::new()));

    assert!(root_access != branch.access());
    assert!(branch.access() != nested.access());
    assert!(
        branch
            .access()
            .same_runtime(&nested.access())
            .expect("runtime identity")
    );
    assert!(root.is_active().expect("root active state"));
    assert!(branch.is_active().expect("branch active state"));
    assert!(nested.is_active().expect("nested active state"));

    let nested_cleanup_order = cleanup_order.clone();
    nested
        .access()
        .on_cleanup(
            move || {
                nested_cleanup_order.borrow_mut().push("nested");
                Ok(())
            },
            handler(root_access),
        )
        .expect("nested cleanup registration");
    let branch_cleanup_order = cleanup_order.clone();
    branch
        .access()
        .on_cleanup(
            move || {
                branch_cleanup_order.borrow_mut().push("branch");
                Ok(())
            },
            handler(root_access),
        )
        .expect("branch cleanup registration");

    root.close()
        .expect("parent close should close descendants first");
    assert_eq!(cleanup_order.borrow().as_slice(), ["nested", "branch"]);
    assert!(!root.is_active().expect("root active state"));
    assert!(!branch.is_active().expect("branch active state"));
    assert!(!nested.is_active().expect("nested active state"));

    nested
        .close()
        .expect("parent-closed child should be inactive");
    nested
        .close()
        .expect("repeated child close should be a no-op");
    branch
        .close()
        .expect("parent-closed branch should be inactive");
    branch
        .close()
        .expect("repeated branch close should be a no-op");
    assert_eq!(cleanup_order.borrow().as_slice(), ["nested", "branch"]);
}

#[test]
fn persistent_child_adapter_supports_child_first_and_repeated_close() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let branch = root
        .access()
        .create_owned_child()
        .expect("persistent branch creation");
    let nested = branch
        .access()
        .create_owned_child()
        .expect("nested branch creation");
    let branch_cleanups = Rc::new(Cell::new(0));
    let nested_cleanups = Rc::new(Cell::new(0));

    let nested_cleanups_for_cleanup = nested_cleanups.clone();
    nested
        .access()
        .on_cleanup(
            move || {
                nested_cleanups_for_cleanup.set(nested_cleanups_for_cleanup.get() + 1);
                Ok(())
            },
            handler(root.access()),
        )
        .expect("nested cleanup registration");
    let branch_cleanups_for_cleanup = branch_cleanups.clone();
    branch
        .access()
        .on_cleanup(
            move || {
                branch_cleanups_for_cleanup.set(branch_cleanups_for_cleanup.get() + 1);
                Ok(())
            },
            handler(root.access()),
        )
        .expect("branch cleanup registration");

    nested.close().expect("child close should succeed");
    nested
        .close()
        .expect("repeated child close should be a no-op");
    assert_eq!(nested_cleanups.get(), 1);
    assert_eq!(branch_cleanups.get(), 0);

    branch.close().expect("branch close should succeed");
    branch
        .close()
        .expect("repeated branch close should be a no-op");
    assert_eq!(nested_cleanups.get(), 1);
    assert_eq!(branch_cleanups.get(), 1);

    root.close()
        .expect("root close should succeed after child close");
    assert_eq!(nested_cleanups.get(), 1);
    assert_eq!(branch_cleanups.get(), 1);
}

#[test]
fn persistent_child_access_rejects_operations_after_adapter_close() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let branch = root
        .access()
        .create_owned_child()
        .expect("persistent branch creation");
    let branch_access = branch.access();

    branch.close().expect("child close");

    assert!(!branch_access.is_active().expect("branch active state"));
    assert!(matches!(
        branch_access.create_child(),
        Err(ReactiveError::NoSuchNode)
    ));
    drop(branch);
    root.close()
        .expect("root close should retain idempotent cleanup");
}

#[cfg(feature = "test-support")]
#[test]
fn owner_churn_removes_released_children_from_parent_registry() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");

    for _ in 0..32 {
        let child = root.create_child().expect("child creation");
        child.close().expect("child close");
        assert_eq!(
            root.runtime_snapshot()
                .expect("runtime snapshot")
                .retained_children,
            0
        );
    }

    root.close().expect("root close");
}

#[cfg(feature = "test-support")]
#[test]
fn owner_churn_reclaims_all_node_slot_allocations() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let handler = root
        .access()
        .error_handler(|_: ()| {})
        .expect("handler registration");

    for _ in 0..32 {
        let child = root.create_child().expect("child creation");
        let access = child.access();
        let effect = access
            .effect(EffectPhase::Normal, || Ok::<(), ()>(()), handler.view())
            .expect("effect creation");
        effect.stop().expect("effect stop");
        let previous = access
            .effect_with_previous(
                EffectPhase::Normal,
                |previous: Option<&i32>| Ok::<i32, ()>(previous.copied().unwrap_or(0)),
                handler.view(),
            )
            .expect("previous effect creation");
        previous.stop().expect("previous effect stop");
        let watch = access
            .watch_getter_with_options(
                EffectPhase::Normal,
                || Ok::<i32, ()>(0),
                |_: &i32, _: Option<&i32>| Ok::<(), ()>(()),
                handler.view(),
                Default::default(),
            )
            .expect("watch creation");
        watch.stop().expect("watch stop");
        let _computed = access
            .computed(|| Ok::<i32, ()>(1), handler.view())
            .expect("computed creation");

        child.close().expect("child close");
        let snapshot = child.runtime_snapshot().expect("runtime snapshot");
        assert_eq!(snapshot.live_typed_slots, 0);
        assert_eq!(snapshot.live_error_slots, 0);
    }

    let snapshot = root.runtime_snapshot().expect("runtime snapshot");
    assert_eq!(snapshot.live_typed_slots, 0);
    assert_eq!(snapshot.live_error_slots, 0);
    root.close().expect("root close");
}

#[cfg(feature = "test-support")]
#[test]
fn persistent_adapter_keeps_storage_after_registry_unlink() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let branch = root
        .access()
        .create_owned_child()
        .expect("persistent child creation");
    let branch_access = branch.access();

    assert_eq!(
        root.runtime_snapshot()
            .expect("runtime snapshot")
            .retained_children,
        1
    );
    branch.close().expect("child close");
    assert_eq!(
        root.runtime_snapshot()
            .expect("runtime snapshot")
            .retained_children,
        0
    );
    assert!(!branch.is_active().expect("branch active state"));
    assert!(
        !branch_access
            .is_active()
            .expect("branch access active state")
    );

    drop(branch);
    root.close().expect("root close");
}
