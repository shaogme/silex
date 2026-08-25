use silex_dom::{
    adapters::ssr::SsrDom,
    diagnostics::error::DomError,
    lifecycle::node_ref::{ClearOutcome, LogicalRefState, NodeRef},
    model::node::ElementSpec,
};

#[test]
fn node_ref_only_stores_opaque_nodes_and_clears() {
    let dom = SsrDom::new();
    let node = dom
        .context()
        .create_text("value")
        .expect("text should be created");
    let reference = NodeRef::new();
    reference.set(node.clone()).expect("set should succeed");
    assert_eq!(reference.get().expect("get should succeed"), Some(node));
    reference.clear().expect("clear should succeed");
    assert_eq!(reference.get().expect("get should succeed"), None);
}

#[test]
fn binding_tokens_only_clear_their_current_generation() {
    let dom = SsrDom::new();
    let context = dom.context();
    let first = context
        .create_element(ElementSpec::new("first"))
        .expect("first element should be created")
        .node()
        .clone();
    let second = context
        .create_element(ElementSpec::new("second"))
        .expect("second element should be created")
        .node()
        .clone();
    let reference = NodeRef::new();

    let first_binding = reference
        .bind_for_mount(first.clone())
        .expect("first binding should succeed");
    assert_eq!(reference.generation(), 1);
    assert_eq!(
        reference.logical_state().expect("state should be readable"),
        LogicalRefState::Bound { generation: 1 }
    );

    let second_binding = reference
        .bind_for_mount(second.clone())
        .expect("second binding should succeed");
    assert_eq!(reference.get().expect("get should succeed"), Some(second));
    assert_eq!(
        reference.logical_state().expect("state should be readable"),
        LogicalRefState::Replaced { generation: 2 }
    );
    assert_eq!(
        first_binding
            .clear_if_current()
            .expect("stale cleanup should be harmless"),
        ClearOutcome::AlreadyReplaced
    );
    assert!(reference.get().expect("get should succeed").is_some());
    assert_eq!(
        second_binding
            .clear_if_current()
            .expect("current cleanup should succeed"),
        ClearOutcome::Cleared
    );
    assert_eq!(
        reference.logical_state().expect("state should be readable"),
        LogicalRefState::Cleared { generation: 2 }
    );
    assert_eq!(
        second_binding
            .clear_if_current()
            .expect("repeated cleanup should be harmless"),
        ClearOutcome::AlreadyCleared
    );
}

#[test]
fn resolve_and_focus_validate_binding_kind_context_and_backend() {
    let dom = SsrDom::new();
    let context = dom.context();
    let element = context
        .create_element(ElementSpec::new("button"))
        .expect("element should be created");
    let text = context.create_text("text").expect("text should be created");

    let unbound = NodeRef::new();
    assert_eq!(
        unbound
            .resolve_element(&context)
            .expect("unbound resolve should be harmless"),
        None
    );
    assert_eq!(
        unbound
            .focus(&context)
            .expect_err("unbound focus should fail"),
        DomError::NotBound
    );

    let element_ref = NodeRef::new();
    element_ref
        .set(element.node().clone())
        .expect("element binding should succeed");
    assert_eq!(
        element_ref
            .resolve_element(&context)
            .expect("element resolve should succeed"),
        Some(element.clone())
    );
    assert_eq!(
        element_ref
            .focus(&context)
            .expect_err("SSR focus should be unsupported"),
        DomError::Unsupported {
            capability: "focus"
        }
    );

    let text_ref = NodeRef::new();
    text_ref.set(text).expect("text binding should succeed");
    assert_eq!(
        text_ref
            .resolve_element(&context)
            .expect_err("text cannot resolve as an element"),
        DomError::WrongNodeKind {
            expected: "element",
            actual: "text",
        }
    );

    let other_dom = SsrDom::new();
    let other_ref = NodeRef::new();
    other_ref
        .set(element.node().clone())
        .expect("cross-context test binding should succeed");
    assert_eq!(
        other_ref
            .resolve_element(&other_dom.context())
            .expect_err("cross-context resolve should fail"),
        DomError::CrossContext {
            expected: other_dom.context().backend_id().value(),
            actual: context.backend_id().value()
        }
    );

    element_ref.clear().expect("clear should succeed");
    assert_eq!(
        element_ref
            .focus(&context)
            .expect_err("cleared focus should fail"),
        DomError::Cleared { generation: 1 }
    );
}
