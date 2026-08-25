use silex_dom::{
    adapters::ssr::{SerializeOptions, SsrDom},
    diagnostics::error::DomError,
    model::node::{ElementSpec, Namespace},
    runtime::tree::{InsertRequest, RangeRequest},
};

#[test]
fn ssr_handles_fragments_ranges_void_and_namespaces() {
    let dom = SsrDom::new();
    let context = dom.context();
    let document = context.document().expect("document should exist");
    let parent = context
        .create_element(ElementSpec::new("main"))
        .expect("parent should exist");
    let fragment = context.create_fragment().expect("fragment should exist");
    let first = context
        .create_comment("a--b")
        .expect("comment should exist");
    let second = context
        .create_element(ElementSpec::new("br"))
        .expect("void element should exist");
    context.append(&fragment, &first).expect("fragment append");
    context
        .append(&fragment, second.node())
        .expect("fragment append");
    context
        .append(parent.node(), &fragment)
        .expect("fragment move");
    let range = context
        .range(RangeRequest {
            parent: parent.node().clone(),
            start: first.clone(),
            end: second.node().clone(),
        })
        .expect("range should validate");
    assert_eq!(range.nodes().expect("range should read").len(), 2);
    let svg = context
        .create_element(ElementSpec::namespaced("svg", Namespace::Svg, false))
        .expect("svg should exist");
    context
        .append(parent.node(), svg.node())
        .expect("svg append");
    context
        .append(document.node(), parent.node())
        .expect("document append");
    assert_eq!(
        dom.serialize(SerializeOptions::default())
            .expect("serialization should work"),
        "<main><!--a- -b--><br><svg xmlns=\"http://www.w3.org/2000/svg\"></svg></main>"
    );
    range.remove().expect("range removal should work");
    assert_eq!(context.children(parent.node()).expect("children").len(), 1);
}

#[test]
fn cross_context_and_wrong_parent_operations_are_structured_errors() {
    let first = SsrDom::new();
    let second = SsrDom::new();
    let first_document = first.context().document().expect("document");
    let second_text = second
        .context()
        .create_text("x")
        .expect("text should exist");
    let error = first
        .context()
        .append(first_document.node(), &second_text)
        .expect_err("cross context append should fail");
    assert!(matches!(error, DomError::CrossContext { .. }));

    let detached = first
        .context()
        .create_text("detached")
        .expect("text should exist");
    assert!(matches!(
        first.context().remove(&detached),
        Err(DomError::NoParent)
    ));
}

#[test]
fn handles_keep_identity_across_queries_but_not_contexts() {
    let first = SsrDom::new();
    let first_context = first.context();
    let first_document = first_context.document().expect("document");
    let text = first_context.create_text("x").expect("text");
    first_context
        .append(first_document.node(), &text)
        .expect("append");

    let same_document = first.context().document().expect("document");
    let queried_child = first_context
        .children(first_document.node())
        .expect("children")
        .pop()
        .expect("child");
    assert!(first_context.same_backend(&first.context()));
    assert_eq!(first_document.node(), same_document.node());
    assert_eq!(text, queried_child);

    let second = SsrDom::new();
    let second_document = second.document().expect("document");
    assert_ne!(first_document.node(), second_document.node());
    assert!(!first_context.same_backend(&second.context()));
}

#[test]
fn ssr_capability_matrix_is_explicit_and_safe() {
    let dom = SsrDom::new();
    let context = dom.context();
    let document = context.document().expect("document");
    let element = context
        .create_element(ElementSpec::new("button"))
        .expect("element");
    let text = context.create_text("text").expect("text");
    let fragment = context.create_fragment().expect("fragment");

    assert!(matches!(
        context.element(document.node()),
        Err(DomError::WrongNodeKind { .. })
    ));
    assert!(matches!(
        context.element(&text),
        Err(DomError::WrongNodeKind { .. })
    ));
    assert!(matches!(
        context.element(&fragment),
        Err(DomError::WrongNodeKind { .. })
    ));
    assert_eq!(
        context.set_text(element.node(), "wrong kind"),
        Err(DomError::WrongNodeKind {
            expected: "text",
            actual: "element",
        })
    );
    assert_eq!(
        context.append(&text, element.node()),
        Err(DomError::CannotContain { parent: "text" })
    );
    assert_eq!(
        context.focus(&element),
        Err(DomError::Unsupported {
            capability: "focus"
        })
    );
}

#[test]
fn insert_before_moves_existing_node_once() {
    let dom = SsrDom::new();
    let context = dom.context();
    let parent = context
        .create_element(ElementSpec::new("div"))
        .expect("parent");
    let first = context.create_text("first").expect("first");
    let second = context.create_text("second").expect("second");
    context.append(parent.node(), &first).expect("append");
    context.append(parent.node(), &second).expect("append");
    context
        .insert_before(InsertRequest::before(parent.node(), &second, &first))
        .expect("move before");
    assert_eq!(
        context.children(parent.node()).expect("children"),
        vec![second, first]
    );
}

#[test]
fn insert_before_relocates_reference_after_detaching_prior_node() {
    let dom = SsrDom::new();
    let context = dom.context();
    let parent = context
        .create_element(ElementSpec::new("div"))
        .expect("parent");
    let first = context.create_text("a").expect("first");
    let second = context.create_text("b").expect("second");
    let reference = context.create_text("c").expect("reference");
    for node in [&first, &second, &reference] {
        context.append(parent.node(), node).expect("append");
    }

    context
        .insert_before(InsertRequest::before(parent.node(), &first, &reference))
        .expect("move before later reference");

    assert_eq!(
        context.children(parent.node()).expect("children"),
        vec![second, first, reference]
    );
}

#[test]
fn insert_before_same_node_is_a_noop() {
    let dom = SsrDom::new();
    let context = dom.context();
    let parent = context
        .create_element(ElementSpec::new("div"))
        .expect("parent");
    let first = context.create_text("a").expect("first");
    let reference = context.create_text("b").expect("reference");
    context.append(parent.node(), &first).expect("append");
    context.append(parent.node(), &reference).expect("append");

    context
        .insert_before(InsertRequest::before(parent.node(), &reference, &reference))
        .expect("same-node insertion");

    assert_eq!(
        context.children(parent.node()).expect("children"),
        vec![first, reference]
    );
}

#[test]
fn insert_before_fragment_preserves_child_order() {
    let dom = SsrDom::new();
    let context = dom.context();
    let parent = context
        .create_element(ElementSpec::new("div"))
        .expect("parent");
    let first = context.create_text("a").expect("first");
    let second = context.create_text("b").expect("second");
    let reference = context.create_text("c").expect("reference");
    for node in [&first, &second, &reference] {
        context.append(parent.node(), node).expect("append");
    }
    let fragment = context.create_fragment().expect("fragment");
    let fragment_first = context.create_text("x").expect("fragment first");
    let fragment_second = context.create_text("y").expect("fragment second");
    context
        .append(&fragment, &fragment_first)
        .expect("fragment append");
    context
        .append(&fragment, &fragment_second)
        .expect("fragment append");

    context
        .insert_before(InsertRequest::before(parent.node(), &fragment, &reference))
        .expect("fragment insertion");

    assert_eq!(
        context.children(parent.node()).expect("children"),
        vec![first, second, fragment_first, fragment_second, reference]
    );
}

#[test]
fn range_move_preserves_a_contiguous_block_and_identity() {
    let dom = SsrDom::new();
    let context = dom.context();
    let parent = context
        .create_element(ElementSpec::new("div"))
        .expect("parent");
    let first_start = context.create_comment("first-start").expect("start");
    let first_node = context.create_text("first").expect("first");
    let first_end = context.create_comment("first-end").expect("end");
    let second_start = context.create_comment("second-start").expect("start");
    let second_node = context.create_text("second").expect("second");
    let second_end = context.create_comment("second-end").expect("end");
    let reference = context
        .create_element(ElementSpec::new("hr"))
        .expect("reference");
    for node in [
        &first_start,
        &first_node,
        &first_end,
        &second_start,
        &second_node,
        &second_end,
        reference.node(),
    ] {
        context.append(parent.node(), node).expect("append");
    }
    let range = context
        .range(RangeRequest {
            parent: parent.node().clone(),
            start: first_start.clone(),
            end: first_end.clone(),
        })
        .expect("range");
    range
        .move_before(parent.node(), reference.node())
        .expect("range move");
    assert_eq!(
        context.children(parent.node()).expect("children"),
        vec![
            second_start,
            second_node,
            second_end,
            first_start.clone(),
            first_node,
            first_end,
            reference.node().clone(),
        ]
    );
    let children_before_failed_move = context.children(parent.node()).expect("children");
    let error = range
        .move_before(parent.node(), &first_start)
        .expect_err("a range cannot move before a reference inside itself");
    assert_eq!(error, DomError::ParentMismatch);
    assert_eq!(
        context.children(parent.node()).expect("children"),
        children_before_failed_move
    );
}
