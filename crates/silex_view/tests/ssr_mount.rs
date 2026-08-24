use silex_core::reactivity::Signal;
use silex_core::{Runtime, SilexError, SilexErrorKind};
use silex_dom::error::CleanupSink;
use silex_dom::ssr::{SerializeOptions, SsrDom};
use silex_view::attribute::{AttributeBuilder, GlobalAttributes, GlobalEventAttributes};
use silex_view::element::Element;
use silex_view::event;
use silex_view::{AnyView, MountedApp, RenderOnlyKeyedListView};
use std::rc::Rc;
use std::{cell::Cell, marker::PhantomData};

fn app(dom: &SsrDom) -> MountedApp {
    let host = dom.document().expect("SSR document").node().clone();
    MountedApp::new(
        Runtime::new(),
        dom.context(),
        host,
        CleanupSink::new(|_| {}),
    )
}

#[test]
fn mount_serializes_attributes_and_nested_text() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context
                .access()
                .error_handler(|_| {})
                .expect("error handler");
            let view = Element::with_child("section", "hello")
                .id("root")
                .class("shell")
                .attr("data-value", "<&");
            context.mount_unit(view, handler.view())
        })
        .expect("mount should succeed");

    assert_eq!(
        dom.serialize(SerializeOptions::default())
            .expect("serialize"),
        "<section class=\"shell\" data-value=\"&lt;&amp;\" id=\"root\">hello</section>"
    );
    assert!(mounted.is_active().expect("active state"));
    mounted.dispose().expect("dispose should succeed");
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn failed_builder_rolls_back_staging_and_remains_retryable() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let cleared = Rc::new(Cell::new(false));
    let cleared_for_assertion = cleared.clone();
    let error = mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let node_ref = context.owner().node_ref();
            context.mount_unit(
                Element::with_child("div", "temporary").node_ref(node_ref.clone()),
                handler.view(),
            )?;
            assert!(
                node_ref
                    .get()
                    .expect("node ref should be readable")
                    .is_some()
            );
            let cleared_for_cleanup = cleared.clone();
            context.owner().on_cleanup(
                Box::new(move || {
                    cleared_for_cleanup.set(
                        node_ref
                            .get()
                            .expect("node ref should be readable")
                            .is_none(),
                    );
                    Ok(())
                }),
                handler.view(),
            )?;
            Err(SilexError::fatal(SilexErrorKind::Framework(
                "intentional builder failure".into(),
            )))
        })
        .expect_err("builder should fail");

    assert!(error.can_retry());
    assert!(!mounted.is_poisoned());
    assert!(cleared_for_assertion.get());
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn ssr_listener_is_recorded_and_owner_cleanup_is_idempotent() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let view = Element::with_child("button", "go").on(event::click, || Ok(()));
            context.mount_unit(view, handler.view())
        })
        .expect("mount should succeed");
    assert_eq!(dom.event_records().len(), 1);
    let record = dom.hydration_records().pop().expect("hydration record");
    assert!(record.target_identity > 0);
    assert_eq!(record.spec.name(), "click");
    let html = dom.serialize(Default::default()).expect("serialize");
    assert!(!html.contains("onclick"));
    assert!(!html.contains("click"));
    mounted.dispose().expect("first dispose");
    mounted.dispose().expect("second dispose");
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn node_ref_tracks_mounted_node_and_clears_on_dispose() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let cleared = Rc::new(Cell::new(false));
    let cleared_for_assertion = cleared.clone();
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let node_ref = context.owner().node_ref();
            context.mount_unit(
                Element::with_child("div", "node").node_ref(node_ref.clone()),
                handler.view(),
            )?;
            assert!(
                node_ref
                    .get()
                    .expect("node ref should be readable")
                    .is_some()
            );
            let cleared_for_cleanup = cleared.clone();
            context.owner().on_cleanup(
                Box::new(move || {
                    cleared_for_cleanup.set(
                        node_ref
                            .get()
                            .expect("node ref should be readable")
                            .is_none(),
                    );
                    Ok(())
                }),
                handler.view(),
            )
        })
        .expect("mount should succeed");
    mounted.dispose().expect("dispose should succeed");
    assert!(cleared_for_assertion.get());
}

#[test]
fn keyed_rows_keep_identity_order_across_reactive_reorder() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let signal: Signal<'_, Vec<i32>> =
                context.access().signal(vec![1, 2, 3]).expect("signal");
            let list = RenderOnlyKeyedListView {
                each: signal,
                key_fn: Rc::new(|value: &i32| *value),
                view_fn: Rc::new(|value: i32, _| {
                    AnyView::from(Element::with_child("li", value.to_string()))
                }),
                error_handler: None,
                _marker: PhantomData,
            };
            context.mount_unit(list, handler.view())?;
            signal.set(vec![3, 2, 1])
        })
        .expect("mount should succeed");

    let html = dom.serialize(Default::default()).expect("serialize");
    assert!(html.find(">3<").expect("first row") < html.find(">2<").expect("second row"));
    assert!(html.find(">2<").expect("second row") < html.find(">1<").expect("third row"));
}

#[test]
fn reactive_attribute_updates_and_cleans_up() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let title = context
                .access()
                .signal(String::from("first"))
                .expect("signal");
            let view = Element::with_child("div", "value").attr("title", title);
            context.mount_unit(view, handler.view())?;
            title.set(String::from("second"))
        })
        .expect("mount should succeed");

    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<div title=\"second\">value</div>"
    );
    mounted.dispose().expect("dispose should succeed");
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}
