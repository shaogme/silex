use silex_core::reactivity::Signal;
use silex_core::{Runtime, SilexError, SilexErrorKind};
use silex_dom::{
    adapters::ssr::{SerializeOptions, SsrDom},
    lifecycle::CleanupSink,
};
use silex_view::app::MountedApp;
use silex_view::attributes::{AttributeBuilder, GlobalAttributes, GlobalEventAttributes};
use silex_view::elements::AnyView;
use silex_view::elements::Element;
use silex_view::events;
use silex_view::flow::RenderOnlyKeyedListView;
use std::rc::Rc;
use std::{cell::Cell, cell::RefCell};

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
                Element::with_child("div", "temporary")
                    .node_ref(node_ref.clone())
                    .on(events::click, || Ok(())),
                handler.view(),
            )?;
            assert_eq!(dom.event_records().len(), 1);
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

    assert!(matches!(
        error.kind(),
        SilexErrorKind::View(view)
            if view.mount_error().is_some_and(|error| error.can_retry())
    ));
    assert!(!mounted.is_poisoned());
    assert!(cleared_for_assertion.get());
    assert!(dom.event_records().is_empty());
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn ssr_listener_is_recorded_and_owner_cleanup_is_idempotent() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let view = Element::with_child("button", "go").on(events::click, || Ok(()));
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
    assert!(dom.event_records().is_empty());
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn owner_cleanup_orders_binding_event_lease_and_dom_removal() {
    let dom = Rc::new(SsrDom::new());
    let mut mounted = app(dom.as_ref());
    let observed = Rc::new(RefCell::new(None));
    let observed_for_assertion = observed.clone();
    let raw_node = Rc::new(RefCell::new(None));
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let node_ref = context.owner().node_ref();
            let node_for_observer = node_ref.clone();
            let node_ref_for_snapshot = node_ref.clone();
            let dom_for_observer = context.dom().clone();
            let ssr_for_observer = dom.clone();
            let raw_node_for_observer = raw_node.clone();
            let observed_for_cleanup = observed.clone();
            context.owner().on_cleanup(
                Box::new(move || {
                    let node = raw_node_for_observer
                        .borrow()
                        .clone()
                        .expect("observer should retain the old node handle");
                    let snapshot = (
                        node_for_observer
                            .get()
                            .expect("node ref should be readable")
                            .is_none(),
                        ssr_for_observer.event_records().is_empty(),
                        dom_for_observer
                            .parent(&node)
                            .expect("parent lookup should succeed")
                            .is_none(),
                    );
                    *observed_for_cleanup.borrow_mut() = Some(snapshot);
                    Ok(())
                }),
                handler.view(),
            )?;
            context.mount_unit(
                Element::with_child("button", "go")
                    .node_ref(node_ref)
                    .on(events::click, || Ok(())),
                handler.view(),
            )?;
            *raw_node.borrow_mut() = node_ref_for_snapshot
                .get()
                .expect("node ref should be readable");
            Ok(())
        })
        .expect("mount should succeed");
    mounted.dispose().expect("dispose should succeed");
    let snapshot = (*observed_for_assertion.borrow()).expect("cleanup should record the order");
    assert!(
        snapshot.0,
        "NodeRef binding must clear before parent cleanup"
    );
    assert!(
        snapshot.1,
        "SSR event lease must be cancelled before parent cleanup"
    );
    assert!(
        snapshot.2,
        "DOM removal must complete before the root observer"
    );
}

#[test]
fn poisoned_mount_does_not_leave_ssr_event_records() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let error = mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            context.mount_unit(
                Element::with_child("button", "panic").on(events::click, || Ok(())),
                handler.view(),
            )?;
            panic!("intentional poison");
        })
        .expect_err("panic should poison the app");
    assert!(matches!(
        error.kind(),
        SilexErrorKind::View(view)
            if view.mount_error().is_some_and(|error| error.is_poisoned())
    ));
    assert!(mounted.is_poisoned());
    assert!(dom.event_records().is_empty());
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
            let list = RenderOnlyKeyedListView::new(
                signal,
                Rc::new(|value: &i32| *value),
                Rc::new(|value: i32, _| {
                    AnyView::from(Element::with_child("li", value.to_string()))
                }),
                None,
            );
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
