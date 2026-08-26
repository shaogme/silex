#![cfg(target_arch = "wasm32")]

use js_sys::Promise;
use silex_core::reactivity::Signal;
use silex_core::{Runtime, RxGet, SilexError, SilexErrorKind};
use silex_dom::{
    adapters::browser::BrowserDom,
    diagnostics::DomError,
    lifecycle::CleanupSink,
    model::{
        DomNode, ElementSpec,
        event::{DomEventBridge, EventKind, EventSpec, PhysicalEventRequest, WindowEventRequest},
    },
    runtime::{HostResourceState, RangeRequest},
};
use silex_view::app::MountedApp;
use silex_view::attributes::{AttributeBuilder, GlobalAttributes, GlobalEventAttributes};
use silex_view::elements::{AnyView, Element};
use silex_view::events::{
    DomEvent, Event as ViewEvent, EventKind as ViewEventKind, bind_window_event,
};
use silex_view::flow::{
    BranchEvaluation, IndexedListView, RenderOnlyKeyedListView, StableBranch, StatefulKeyedListView,
};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{
    Document, Element as RawElement, Event, HtmlIFrameElement, HtmlInputElement, MouseEvent,
};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
    web_sys::window()
        .expect("browser window should exist")
        .document()
        .expect("browser document should exist")
}

fn test_host(browser: &BrowserDom) -> (RawElement, DomNode) {
    let raw_host = document()
        .create_element("div")
        .expect("test host should be creatable");
    raw_host
        .set_attribute("data-silex-view-test", "true")
        .expect("test host attribute should be set");
    document()
        .body()
        .expect("document body should exist")
        .append_child(&raw_host)
        .expect("test host should attach");
    let host = browser
        .from_web_sys_node(raw_host.clone().into())
        .expect("test host should have an opaque node handle");
    (raw_host, host)
}

#[wasm_bindgen_test]
fn browser_rejects_foreign_documents_and_cross_adapter_handles() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let second_adapter = BrowserDom::new(document());
    let document_handle = browser.document().expect("document handle");
    assert!(matches!(
        second_adapter.context().element(document_handle.node()),
        Err(DomError::CrossContext { .. })
    ));

    let frame = document()
        .create_element("iframe")
        .expect("iframe should be creatable")
        .dyn_into::<HtmlIFrameElement>()
        .expect("iframe should have an HTML iframe interface");
    document()
        .body()
        .expect("document body should exist")
        .append_child(&frame)
        .expect("iframe should attach");
    let foreign_document = frame
        .content_document()
        .expect("iframe should expose a document");
    let foreign_node: web_sys::Node = foreign_document
        .create_element("div")
        .expect("foreign element should be creatable")
        .into();
    assert!(matches!(
        browser.from_web_sys_node(foreign_node),
        Err(DomError::CrossContext { .. })
    ));
    document()
        .body()
        .expect("document body should exist")
        .remove_child(&frame)
        .expect("iframe should be removable");
}

#[wasm_bindgen_test]
fn browser_focus_validates_attachment_and_node_kinds() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let context = browser.context();
    let detached = context
        .create_element(ElementSpec::new("button"))
        .expect("button should be creatable");
    let text = context
        .create_text("text")
        .expect("text should be creatable");
    let fragment = context
        .create_fragment()
        .expect("fragment should be creatable");
    let document_handle = context.document().expect("document handle");

    assert!(matches!(
        context.focus(&detached),
        Err(DomError::Detached { .. })
    ));
    assert!(matches!(
        context.element(document_handle.node()),
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

    let body = context
        .document_body()
        .expect("document body capability should be available")
        .expect("document body should exist");
    context
        .append(body.node(), detached.node())
        .expect("button should attach");
    context
        .focus(&detached)
        .expect("attached button should focus");
    let active = context
        .active_element()
        .expect("active element should be queryable")
        .expect("button should be active");
    assert!(active.node().is_same_node(detached.node()));
    context
        .remove(detached.node())
        .expect("button should detach");
    assert!(matches!(
        context.focus(&detached),
        Err(DomError::Detached { .. })
    ));
}

#[wasm_bindgen_test]
fn browser_element_and_window_resources_cancel_independently() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let context = browser.context();
    let element = context.element(&host).expect("host should be an element");
    let element_calls = Rc::new(Cell::new(0));
    let window_calls = Rc::new(Cell::new(0));
    let element_calls_for_bridge = element_calls.clone();
    let window_calls_for_bridge = window_calls.clone();
    let element_bridge: Rc<dyn DomEventBridge> = Rc::new(move |_| {
        element_calls_for_bridge.set(element_calls_for_bridge.get() + 1);
        Ok(())
    });
    let window_bridge: Rc<dyn DomEventBridge> = Rc::new(move |_| {
        window_calls_for_bridge.set(window_calls_for_bridge.get() + 1);
        Ok(())
    });
    let element_resource = context
        .listen(
            PhysicalEventRequest::new(&element, EventSpec::new("click", EventKind::Mouse))
                .with_bridge(element_bridge),
        )
        .expect("element listener should attach");
    let window_resource = context
        .listen_window(
            WindowEventRequest::new(EventSpec::new("resize", EventKind::Custom))
                .with_bridge(window_bridge),
        )
        .expect("window listener should attach");

    raw_host
        .dispatch_event(&Event::new("click").expect("click event"))
        .expect("element event should dispatch");
    web_sys::window()
        .expect("window should exist")
        .dispatch_event(&Event::new("resize").expect("resize event"))
        .expect("window event should dispatch");
    assert_eq!(element_calls.get(), 1);
    assert_eq!(window_calls.get(), 1);

    element_resource
        .cancel()
        .expect("element listener should cancel");
    element_resource
        .cancel()
        .expect("repeated element cancellation should be inert");
    assert_eq!(element_resource.state(), HostResourceState::Cancelled);
    raw_host
        .dispatch_event(&Event::new("click").expect("click event"))
        .expect("element event should dispatch after cancellation");
    web_sys::window()
        .expect("window should exist")
        .dispatch_event(&Event::new("resize").expect("resize event"))
        .expect("window event should dispatch after element cancellation");
    assert_eq!(element_calls.get(), 1);
    assert_eq!(window_calls.get(), 2);

    window_resource
        .cancel()
        .expect("window listener should cancel");
    window_resource
        .cancel()
        .expect("repeated window cancellation should be inert");
    assert_eq!(window_resource.state(), HostResourceState::Cancelled);
    web_sys::window()
        .expect("window should exist")
        .dispatch_event(&Event::new("resize").expect("resize event"))
        .expect("window event should dispatch after cancellation");
    assert_eq!(window_calls.get(), 2);
    remove_host(&raw_host);
}

fn app(browser: &BrowserDom, host: DomNode) -> MountedApp {
    MountedApp::new(
        Runtime::new(),
        browser.context(),
        host,
        CleanupSink::new(|_| {}),
    )
}

#[wasm_bindgen_test(async)]
async fn browser_mount_dom_action_focuses_node_ref_and_closes_events() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);
    let element_calls = Rc::new(Cell::new(0));
    let window_calls = Rc::new(Cell::new(0));

    mounted
        .mount({
            let element_calls = element_calls.clone();
            let window_calls = window_calls.clone();
            move |context| {
                let handler = context.access().error_handler(|_| {}).expect("handler");
                let node_ref = context.owner().node_ref();
                let action = context.dom_action();
                let action_for_element = action.clone();
                let node_ref_for_element = node_ref.clone();
                let window_calls_for_attribute = window_calls.clone();
                let element_calls_for_handler = element_calls.clone();
                let view = Element::new("input")
                    .node_ref(node_ref)
                    .on_click(move |_| {
                        action_for_element.with_context(|dom| node_ref_for_element.focus(dom))?;
                        element_calls_for_handler.set(element_calls_for_handler.get() + 1);
                        Ok(())
                    })
                    .apply(silex_view::attributes::AttrOp::custom(
                        move |_element, mount_context| {
                            let window_calls_for_handler = window_calls_for_attribute.clone();
                            bind_window_event(
                                mount_context,
                                ViewEvent::new("resize", ViewEventKind::Custom),
                                move |_| {
                                    window_calls_for_handler
                                        .set(window_calls_for_handler.get() + 1);
                                    Ok(())
                                },
                                mount_context.error_handler(),
                            )
                        },
                    ));
                context.mount_unit(view, handler.view())
            }
        })
        .expect("focus view should mount");

    let input = query_element(&raw_host, "input");
    input
        .dispatch_event(&MouseEvent::new("click").expect("click event"))
        .expect("input click should dispatch");
    web_sys::window()
        .expect("window should exist")
        .dispatch_event(&Event::new("resize").expect("resize event"))
        .expect("window event should dispatch");
    flush_browser_tasks().await;
    let active: web_sys::Node = document()
        .active_element()
        .expect("active element should exist")
        .into();
    let input_node: web_sys::Node = input.clone().into();
    assert!(active.is_same_node(Some(&input_node)));
    assert_eq!(element_calls.get(), 1);
    assert_eq!(window_calls.get(), 1);

    mounted.dispose().expect("dispose should succeed");
    input
        .dispatch_event(&MouseEvent::new("click").expect("click event"))
        .expect("element event after dispose should dispatch");
    web_sys::window()
        .expect("window should exist")
        .dispatch_event(&Event::new("resize").expect("resize event"))
        .expect("window event after dispose should dispatch");
    flush_browser_tasks().await;
    assert_eq!(element_calls.get(), 1);
    assert_eq!(window_calls.get(), 1);
    remove_host(&raw_host);
}

fn remove_host(host: &RawElement) {
    let Some(parent) = host.parent_node() else {
        return;
    };
    parent
        .remove_child(host)
        .expect("test host should be removable");
}

#[wasm_bindgen_test]
fn browser_dom_range_moves_an_existing_multi_node_block() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let context = browser.context();
    let start = context
        .create_comment("range-start")
        .expect("range start should be creatable");
    let first = context
        .create_element(ElementSpec::new("p"))
        .expect("first element should be creatable")
        .node()
        .clone();
    let second = context
        .create_element(ElementSpec::new("span"))
        .expect("second element should be creatable")
        .node()
        .clone();
    let end = context
        .create_comment("range-end")
        .expect("range end should be creatable");
    let reference = context
        .create_element(ElementSpec::new("button"))
        .expect("reference should be creatable")
        .node()
        .clone();
    for node in [&start, &first, &second, &end, &reference] {
        context
            .append(&host, node)
            .expect("node should attach to host");
    }
    let range = context
        .range(RangeRequest {
            parent: host.clone(),
            start: start.clone(),
            end: end.clone(),
        })
        .expect("range should be valid");
    range
        .move_before(&host, &reference)
        .expect("range should move before the reference");
    assert_eq!(
        raw_host.inner_html(),
        "<!--range-start--><p></p><span></span><!--range-end--><button></button>"
    );
    remove_host(&raw_host);
}

async fn flush_browser_tasks() {
    for _ in 0..4 {
        JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
            .await
            .expect("browser task should resolve");
    }
}

fn query_element(host: &RawElement, selector: &str) -> RawElement {
    host.query_selector(selector)
        .expect("selector should be valid")
        .expect("element should be mounted")
}

fn list_elements(host: &RawElement) -> Vec<RawElement> {
    let nodes = host
        .query_selector_all("li")
        .expect("list selector should be valid");
    (0..nodes.length())
        .filter_map(|index| nodes.item(index))
        .filter_map(|node| node.dyn_into::<RawElement>().ok())
        .collect()
}

fn selected_elements(host: &RawElement, selector: &str) -> Vec<RawElement> {
    let nodes = host
        .query_selector_all(selector)
        .expect("selector should be valid");
    (0..nodes.length())
        .filter_map(|index| nodes.item(index))
        .filter_map(|node| node.dyn_into::<RawElement>().ok())
        .collect()
}

#[wasm_bindgen_test(async)]
async fn browser_mounts_and_updates_reactive_attributes() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount({
            move |context| {
                let handler = context.access().error_handler(|_| {}).expect("handler");
                let title = context
                    .access()
                    .signal(String::from("before"))
                    .expect("title signal");
                let selected = context.access().signal(false).expect("class signal");
                let view = Element::with_child("button", "update")
                    .id("reactive-button")
                    .class("base")
                    .class_toggle("selected", selected)
                    .attr("data-title", title)
                    .style("color:red")
                    .on_click(move |_| {
                        title.set(String::from("after"))?;
                        selected.set(true)
                    });
                context.mount_unit(view, handler.view())
            }
        })
        .expect("reactive view should mount");

    let button = query_element(&raw_host, "button");
    assert_eq!(button.id(), "reactive-button");
    assert_eq!(
        button.get_attribute("data-title").as_deref(),
        Some("before")
    );
    assert_eq!(button.get_attribute("class").as_deref(), Some("base"));
    let event = MouseEvent::new("click").expect("click event should be creatable");
    button
        .dispatch_event(&event)
        .expect("update event should dispatch");
    flush_browser_tasks().await;
    assert_eq!(button.get_attribute("data-title").as_deref(), Some("after"));
    assert_eq!(
        button.get_attribute("class").as_deref(),
        Some("base selected")
    );
    assert_eq!(
        button.get_attribute("style").as_deref(),
        Some("color: red;")
    );

    mounted.dispose().expect("dispose should succeed");
    assert!(!button.has_attribute("data-title"));
    assert_eq!(raw_host.child_element_count(), 0);
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_bind_value_updates_reactive_text_from_input_event() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let value = context
                .access()
                .signal(String::from("before"))
                .expect("value signal");
            let view = vec![
                Element::new("input").bind_value(value),
                Element::with_child("output", value),
            ];
            context.mount_unit(view, handler.view())
        })
        .expect("bound input should mount");

    let input = query_element(&raw_host, "input")
        .dyn_into::<HtmlInputElement>()
        .expect("bound input should have an HTML input interface");
    let output = query_element(&raw_host, "output");
    assert_eq!(input.value(), "before");
    assert_eq!(output.text_content().as_deref(), Some("before"));

    input.set_value("after");
    input
        .dispatch_event(&Event::new("input").expect("input event should be creatable"))
        .expect("input event should dispatch");
    flush_browser_tasks().await;

    assert_eq!(output.text_content().as_deref(), Some("after"));
    mounted.dispose().expect("dispose should succeed");
    remove_host(&raw_host);
}

#[wasm_bindgen_test]
fn browser_preserves_empty_value_and_boolean_property_types() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let view = vec![
                Element::new("input").prop("value", ""),
                Element::new("input").prop("checked", true),
            ];
            context.mount_unit(view, handler.view())
        })
        .expect("property views should mount");

    let empty_input = query_element(&raw_host, "input:nth-of-type(1)")
        .dyn_into::<HtmlInputElement>()
        .expect("empty input should have an HTML input interface");
    let checked_input = query_element(&raw_host, "input:nth-of-type(2)")
        .dyn_into::<HtmlInputElement>()
        .expect("checked input should have an HTML input interface");
    assert_eq!(empty_input.value(), "");
    assert!(checked_input.checked());

    mounted.dispose().expect("dispose should succeed");
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_dispatches_events_and_removes_listener_on_dispose() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);
    let calls = Rc::new(Cell::new(0));
    let event_target_identity = Rc::new(Cell::new(0_u64));

    mounted
        .mount({
            let calls = calls.clone();
            let event_target_identity = event_target_identity.clone();
            move |context| {
                let handler = context.access().error_handler(|_| {}).expect("handler");
                let calls_for_handler = calls.clone();
                let identity_for_handler = event_target_identity.clone();
                let view =
                    Element::with_child("button", "click").on_click(move |event: DomEvent| {
                        calls_for_handler.set(calls_for_handler.get() + 1);
                        identity_for_handler.set(event.target().identity());
                        assert_eq!(event.mouse_data().expect("mouse data").button(), 0);
                        Ok(())
                    });
                context.mount_unit(view, handler.view())
            }
        })
        .expect("event view should mount");

    let button = query_element(&raw_host, "button");
    let event = MouseEvent::new("click").expect("click event should be creatable");
    button
        .dispatch_event(&event)
        .expect("click event should dispatch");
    flush_browser_tasks().await;
    assert_eq!(calls.get(), 1);
    assert!(event_target_identity.get() > 0);

    mounted.dispose().expect("dispose should succeed");
    button
        .dispatch_event(&event)
        .expect("click after dispose should dispatch");
    flush_browser_tasks().await;
    assert_eq!(calls.get(), 1);
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_dynamic_views_and_keyed_rows_keep_dom_identity() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount({
            move |context| {
                let handler = context.access().error_handler(|_| {}).expect("handler");
                let dynamic_value = context
                    .access()
                    .signal(String::from("before"))
                    .expect("dynamic signal");
                let dynamic = move || {
                    Element::with_child(
                        "p",
                        dynamic_value
                            .get()
                            .expect("dynamic signal should remain active"),
                    )
                };
                context.mount_unit(dynamic, handler.view())?;

                let values: Signal<'_, Vec<i32>> =
                    context.access().signal(vec![1, 2, 3]).expect("list signal");
                let list = StatefulKeyedListView::new(
                    values,
                    Rc::new(|value: &i32| *value),
                    Rc::new(|value: i32, _index, updater| {
                        assert!(updater.bind(|_, _| Ok(())));
                        AnyView::from(Element::with_child("li", value.to_string()))
                    }),
                    None,
                );
                context.mount_unit(list, handler.view())?;
                let update = Element::with_child("button", "reorder").on_click(move |_| {
                    dynamic_value.set(String::from("after"))?;
                    values.set(vec![3, 1, 2])
                });
                context.mount_unit(update, handler.view())
            }
        })
        .expect("dynamic and keyed views should mount");

    assert!(
        raw_host
            .text_content()
            .unwrap_or_default()
            .contains("before")
    );
    let initial = list_elements(&raw_host);
    assert_eq!(
        initial
            .iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3"]
    );
    let update = query_element(&raw_host, "button");
    let event = MouseEvent::new("click").expect("click event should be creatable");
    update
        .dispatch_event(&event)
        .expect("reorder event should dispatch");
    flush_browser_tasks().await;

    assert!(
        raw_host
            .text_content()
            .unwrap_or_default()
            .contains("after")
    );
    let rows = list_elements(&raw_host);
    assert_eq!(
        rows.iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["3", "1", "2"],
        "unexpected keyed DOM after reorder: {}",
        raw_host.inner_html()
    );
    assert!(rows[0].is_same_node(Some(&initial[2])));
    assert!(rows[1].is_same_node(Some(&initial[0])));
    assert!(rows[2].is_same_node(Some(&initial[1])));
    mounted.dispose().expect("dispose should succeed");
    assert_eq!(raw_host.child_element_count(), 0);
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_stateful_keyed_rows_keep_multi_node_identity_events_and_nested_dynamic_views() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);
    let row_clicks = Rc::new(Cell::new(0_u32));

    mounted
        .mount({
            let row_clicks = row_clicks.clone();
            move |context| {
                let handler = context.access().error_handler(|_| {}).expect("handler");
                let values: Signal<'_, Vec<i32>> =
                    context.access().signal(vec![1, 2, 3]).expect("list signal");
                let labels: Signal<'_, Vec<String>> = context
                    .access()
                    .signal(vec![
                        String::from("one"),
                        String::from("two"),
                        String::from("three"),
                    ])
                    .expect("nested signal");
                let list_values = values;
                let list_labels = labels;
                let list_clicks = row_clicks.clone();
                let list = StatefulKeyedListView::new(
                    list_values,
                    Rc::new(|value: &i32| *value),
                    Rc::new(move |value: i32, _index, updater| {
                        assert!(updater.bind(|_, _| Ok(())));
                        let nested_labels = list_labels;
                        let row_clicks = list_clicks.clone();
                        let row_button = Element::with_child("button", format!("select-{value}"))
                            .attr("data-row", value.to_string())
                            .on_click(move |_| {
                                row_clicks.set(row_clicks.get() + 1);
                                Ok(())
                            });
                        let nested = Element::with_child("span", move || {
                            nested_labels
                                .get()
                                .expect("nested signal should remain active")
                                .get((value - 1) as usize)
                                .cloned()
                                .expect("nested label should exist")
                        });
                        AnyView::from(vec![
                            Element::with_child("li", format!("row-{value}")),
                            nested,
                            row_button,
                        ])
                    }),
                    None,
                );
                context.mount_unit(list, handler.view())?;
                let reorder_values = values;
                let reorder_labels = labels;
                let reorder_second = Rc::new(Cell::new(false));
                let reorder = Element::with_child("button", "reorder")
                    .id("multi-node-reorder")
                    .on_click(move |_| {
                        if reorder_second.get() {
                            reorder_second.set(false);
                            reorder_labels.set(vec![
                                String::from("TWO"),
                                String::from("THREE"),
                                String::from("ONE"),
                            ])?;
                            reorder_values.set(vec![2, 3, 1])
                        } else {
                            reorder_second.set(true);
                            reorder_labels.set(vec![
                                String::from("ONE"),
                                String::from("TWO"),
                                String::from("THREE"),
                            ])?;
                            reorder_values.set(vec![3, 1, 2])
                        }
                    });
                context.mount_unit(reorder, handler.view())
            }
        })
        .expect("multi-node keyed view should mount");

    let initial_rows = list_elements(&raw_host);
    let initial_spans = selected_elements(&raw_host, "span");
    assert_eq!(initial_rows.len(), 3);
    assert_eq!(initial_spans.len(), 3);
    assert_eq!(
        initial_spans
            .iter()
            .map(|span| span.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["one", "two", "three"]
    );

    let first_button = query_element(&raw_host, "button[data-row='1']");
    first_button
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("row event should dispatch");
    flush_browser_tasks().await;
    assert_eq!(row_clicks.get(), 1);

    query_element(&raw_host, "#multi-node-reorder")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("reorder event should dispatch");
    flush_browser_tasks().await;

    let rows = list_elements(&raw_host);
    let spans = selected_elements(&raw_host, "span");
    assert_eq!(
        rows.iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["row-3", "row-1", "row-2"],
        "multi-node keyed reorder lost row content: {}",
        raw_host.inner_html()
    );
    assert_eq!(spans.len(), 3);
    assert_eq!(
        spans
            .iter()
            .map(|span| span.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["THREE", "ONE", "TWO"]
    );
    assert!(rows[0].is_same_node(Some(&initial_rows[2])));
    assert!(rows[1].is_same_node(Some(&initial_rows[0])));
    assert!(rows[2].is_same_node(Some(&initial_rows[1])));
    assert!(spans[0].is_same_node(Some(&initial_spans[2])));
    assert!(spans[1].is_same_node(Some(&initial_spans[0])));
    assert!(spans[2].is_same_node(Some(&initial_spans[1])));

    query_element(&raw_host, "button[data-row='3']")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("moved row event should dispatch");
    flush_browser_tasks().await;
    assert_eq!(row_clicks.get(), 2);

    query_element(&raw_host, "#multi-node-reorder")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("second reorder event should dispatch");
    flush_browser_tasks().await;
    let twice_rows = list_elements(&raw_host);
    let twice_spans = selected_elements(&raw_host, "span");
    assert_eq!(
        twice_rows
            .iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["row-2", "row-3", "row-1"]
    );
    assert_eq!(
        twice_spans
            .iter()
            .map(|span| span.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["THREE", "ONE", "TWO"]
    );
    assert!(twice_rows[0].is_same_node(Some(&initial_rows[1])));
    assert!(twice_rows[1].is_same_node(Some(&initial_rows[2])));
    assert!(twice_rows[2].is_same_node(Some(&initial_rows[0])));
    assert!(twice_spans[0].is_same_node(Some(&initial_spans[1])));
    assert!(twice_spans[1].is_same_node(Some(&initial_spans[2])));
    assert!(twice_spans[2].is_same_node(Some(&initial_spans[0])));

    query_element(&raw_host, "button[data-row='2']")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("second moved row event should dispatch");
    flush_browser_tasks().await;
    assert_eq!(row_clicks.get(), 3);

    mounted.dispose().expect("dispose should succeed");
    assert_eq!(raw_host.child_element_count(), 0);
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_stateful_keyed_duplicate_keys_keep_previous_rows() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);
    let errors = Rc::new(Cell::new(0_u32));

    mounted
        .mount({
            let errors = errors.clone();
            move |context| {
                let errors_for_handler = errors.clone();
                let handler = context
                    .access()
                    .error_handler(move |_| errors_for_handler.set(errors_for_handler.get() + 1))
                    .expect("handler");
                let values: Signal<'_, Vec<i32>> =
                    context.access().signal(vec![1, 2]).expect("list signal");
                let list_values = values;
                let list = StatefulKeyedListView::new(
                    list_values,
                    Rc::new(|value: &i32| *value),
                    Rc::new(|value: i32, _index, updater| {
                        assert!(updater.bind(|_, _| Ok(())));
                        AnyView::from(Element::with_child("li", value.to_string()))
                    }),
                    None,
                );
                context.mount_unit(list, handler.view())?;
                let update_values = values;
                let update = Element::with_child("button", "duplicate")
                    .on_click(move |_| update_values.set(vec![1, 1]));
                context.mount_unit(update, handler.view())
            }
        })
        .expect("duplicate-key view should mount");

    let initial_rows = list_elements(&raw_host);
    assert_eq!(initial_rows.len(), 2);
    query_element(&raw_host, "button")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("duplicate-key event should dispatch");
    flush_browser_tasks().await;

    assert!(errors.get() > 0);
    let rows = list_elements(&raw_host);
    assert_eq!(
        rows.iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["1", "2"]
    );
    assert!(rows[0].is_same_node(Some(&initial_rows[0])));
    assert!(rows[1].is_same_node(Some(&initial_rows[1])));
    mounted.dispose().expect("dispose should succeed");
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_stateful_keyed_insert_delete_preserves_reused_identity() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount(move |context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let values: Signal<'_, Vec<i32>> =
                context.access().signal(vec![1, 2]).expect("list signal");
            let list_values = values;
            let list = StatefulKeyedListView::new(
                list_values,
                Rc::new(|value: &i32| *value),
                Rc::new(|value: i32, _index, updater| {
                    assert!(updater.bind(|_, _| Ok(())));
                    AnyView::from(Element::with_child("li", format!("row-{value}")))
                }),
                None,
            );
            context.mount_unit(list, handler.view())?;
            let insert_values = values;
            let insert = Element::with_child("button", "insert")
                .on_click(move |_| insert_values.set(vec![2, 3]));
            context.mount_unit(insert, handler.view())?;
            let delete_values = values;
            let delete = Element::with_child("button", "delete")
                .on_click(move |_| delete_values.set(vec![3]));
            context.mount_unit(delete, handler.view())
        })
        .expect("insert/delete view should mount");

    let initial_rows = list_elements(&raw_host);
    assert_eq!(initial_rows.len(), 2);
    query_element(&raw_host, "button")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("insert event should dispatch");
    flush_browser_tasks().await;
    let inserted_rows = list_elements(&raw_host);
    assert_eq!(
        inserted_rows
            .iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["row-2", "row-3"]
    );
    assert!(inserted_rows[0].is_same_node(Some(&initial_rows[1])));

    query_element(&raw_host, "button:nth-of-type(2)")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("delete event should dispatch");
    flush_browser_tasks().await;
    let deleted_rows = list_elements(&raw_host);
    assert_eq!(
        deleted_rows
            .iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["row-3"]
    );
    assert!(deleted_rows[0].is_same_node(Some(&inserted_rows[1])));
    mounted.dispose().expect("dispose should succeed");
    remove_host(&raw_host);
}

#[derive(Clone)]
struct RollbackRow {
    id: u8,
    label: &'static str,
}

#[wasm_bindgen_test(async)]
async fn browser_mounts_void_svg_property_and_node_ref_contracts() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);
    let cleared = Rc::new(Cell::new(false));
    let cleared_for_assertion = cleared.clone();

    mounted
        .mount(move |context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let node_ref = context.owner().node_ref();
            let node_ref_for_assertion = node_ref.clone();
            let input = Element::new("input")
                .prop("value", "property value")
                .attr("data-value", "<&")
                .node_ref(node_ref);
            context.mount_unit(
                vec![input, Element::new_svg("svg"), Element::new_svg("path")],
                handler.view(),
            )?;
            assert!(
                node_ref_for_assertion
                    .get()
                    .expect("node ref should be readable")
                    .is_some()
            );
            let cleared_for_cleanup = cleared.clone();
            context.owner().on_cleanup(
                Box::new(move || {
                    cleared_for_cleanup.set(
                        node_ref_for_assertion
                            .get()
                            .expect("node ref should be readable")
                            .is_none(),
                    );
                    Ok(())
                }),
                handler.view(),
            )
        })
        .expect("void and SVG views should mount");

    let input = query_element(&raw_host, "input")
        .dyn_into::<HtmlInputElement>()
        .expect("input should have an input element interface");
    assert_eq!(input.value(), "property value");
    assert_eq!(input.get_attribute("data-value").as_deref(), Some("<&"));
    assert!(input.get_attribute("value").is_none());
    let svg = query_element(&raw_host, "svg");
    assert_eq!(
        svg.namespace_uri().as_deref(),
        Some("http://www.w3.org/2000/svg")
    );
    let path = query_element(&raw_host, "path");
    assert_eq!(
        path.namespace_uri().as_deref(),
        Some("http://www.w3.org/2000/svg")
    );
    mounted.dispose().expect("dispose should succeed");
    assert!(cleared_for_assertion.get());
    assert_eq!(raw_host.child_element_count(), 0);
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_indexed_list_updates_by_position_and_disposes_removed_rows() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let values: Signal<'_, Vec<i32>> =
                context.access().signal(vec![1, 2]).expect("list signal");
            let list = IndexedListView::new(
                values,
                Rc::new(|value: i32, index| {
                    AnyView::from(Element::with_child("li", format!("{index}:{value}")))
                }),
            );
            context.mount_unit(list, handler.view())?;
            let update_values = values;
            let update = Element::with_child("button", "update")
                .on_click(move |_| update_values.set(vec![3, 4, 5]));
            context.mount_unit(update, handler.view())
        })
        .expect("indexed list should mount");

    // Index uses render-only rows: position is the identity, so a changed
    // item is rendered into fresh content instead of preserving its element.
    assert_eq!(list_elements(&raw_host).len(), 2);
    query_element(&raw_host, "button")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("update event should dispatch");
    flush_browser_tasks().await;

    let rows = list_elements(&raw_host);
    assert_eq!(
        rows.iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["0:3", "1:4", "2:5"]
    );
    assert_eq!(rows.len(), 3);
    assert!(!raw_host.inner_html().contains("0:1"));
    assert!(!raw_host.inner_html().contains("1:2"));
    mounted.dispose().expect("dispose should succeed");
    assert_eq!(raw_host.child_element_count(), 0);
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_render_only_keyed_list_reorders_content() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let values: Signal<'_, Vec<i32>> =
                context.access().signal(vec![1, 2, 3]).expect("list signal");
            let list = RenderOnlyKeyedListView::new(
                values,
                Rc::new(|value: &i32| *value),
                Rc::new(|value: i32, _| {
                    AnyView::from(Element::with_child("li", value.to_string()))
                }),
                None,
            );
            context.mount_unit(list, handler.view())?;
            let reorder_values = values;
            let reorder = Element::with_child("button", "reorder")
                .on_click(move |_| reorder_values.set(vec![3, 1, 2]));
            context.mount_unit(reorder, handler.view())
        })
        .expect("render-only keyed list should mount");

    query_element(&raw_host, "button")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("reorder event should dispatch");
    flush_browser_tasks().await;
    // Render-only keyed rows preserve keyed ordering, while stateful keyed
    // rows are the API that preserves DOM identity and local row state.
    let rows = list_elements(&raw_host);
    assert_eq!(
        rows.iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["3", "1", "2"]
    );
    assert_eq!(rows.len(), 3);
    mounted.dispose().expect("dispose should succeed");
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_stable_branch_reuses_same_key_and_replaces_changed_key() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let key = context.access().signal(1_usize).expect("key signal");
            let label = context
                .access()
                .signal(String::from("first"))
                .expect("label signal");
            context.mount_unit(
                StableBranch::new(
                    move || Ok(BranchEvaluation::new(key.get()?, label.get()?)),
                    |evaluation, _| {
                        let (key, label) = evaluation.into_parts();
                        AnyView::from(Element::with_child("output", format!("{key}:{label}")))
                    },
                ),
                handler.view(),
            )?;
            let same_key_label = label;
            let same_key = Element::with_child("button", "same-key")
                .on_click(move |_| same_key_label.set(String::from("updated")));
            context.mount_unit(same_key, handler.view())?;
            let next_key = key;
            let next_label = label;
            let changed_key = Element::with_child("button", "changed-key").on_click(move |_| {
                next_label.set(String::from("second"))?;
                next_key.set(2)
            });
            context.mount_unit(changed_key, handler.view())
        })
        .expect("stable branch should mount");

    let initial = query_element(&raw_host, "output");
    assert_eq!(initial.text_content().as_deref(), Some("1:first"));
    query_element(&raw_host, "button")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("same-key event should dispatch");
    flush_browser_tasks().await;
    let same_key = query_element(&raw_host, "output");
    assert_eq!(same_key.text_content().as_deref(), Some("1:first"));
    assert!(same_key.is_same_node(Some(&initial)));

    query_element(&raw_host, "button:nth-of-type(2)")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("changed-key event should dispatch");
    flush_browser_tasks().await;
    let changed_key = query_element(&raw_host, "output");
    assert_eq!(changed_key.text_content().as_deref(), Some("2:second"));
    assert!(!changed_key.is_same_node(Some(&initial)));
    mounted.dispose().expect("dispose should succeed");
    remove_host(&raw_host);
}

#[wasm_bindgen_test(async)]
async fn browser_stateful_keyed_updater_panic_preserves_previous_dom_and_order() {
    let browser = BrowserDom::from_window().expect("browser DOM should be available");
    let (raw_host, host) = test_host(&browser);
    let mut mounted = app(&browser, host);
    let errors = Rc::new(Cell::new(0_u32));

    mounted
        .mount({
            let errors = errors.clone();
            move |context| {
                let errors_for_handler = errors.clone();
                let handler = context
                    .access()
                    .error_handler(move |_| errors_for_handler.set(errors_for_handler.get() + 1))
                    .expect("handler");
                let values: Signal<'_, Vec<RollbackRow>> = context
                    .access()
                    .signal(vec![
                        RollbackRow {
                            id: 1,
                            label: "one",
                        },
                        RollbackRow {
                            id: 2,
                            label: "two",
                        },
                        RollbackRow {
                            id: 3,
                            label: "three",
                        },
                    ])
                    .expect("rollback signal");
                let list_values = values;
                let list = StatefulKeyedListView::new(
                    list_values,
                    Rc::new(|value: &RollbackRow| value.id),
                    Rc::new(|value: RollbackRow, _index, updater| {
                        let label = value.label;
                        assert!(updater.bind(|next: RollbackRow, _| {
                            if next.label == "error" {
                                return Err(SilexError::fatal(SilexErrorKind::Framework(
                                    "intentional updater error".into(),
                                )));
                            }
                            assert_ne!(next.label, "panic", "intentional updater panic");
                            Ok(())
                        }));
                        AnyView::from(Element::with_child("li", label))
                    }),
                    None,
                );
                context.mount_unit(list, handler.view())?;
                let error_values = values;
                let error_update = Element::with_child("button", "error")
                    .id("error-reorder")
                    .on_click(move |_| {
                        error_values.set(vec![
                            RollbackRow {
                                id: 2,
                                label: "error",
                            },
                            RollbackRow {
                                id: 1,
                                label: "one-updated",
                            },
                            RollbackRow {
                                id: 3,
                                label: "three-updated",
                            },
                        ])
                    });
                context.mount_unit(error_update, handler.view())?;
                let update_values = values;
                let update = Element::with_child("button", "panic")
                    .id("panic-reorder")
                    .on_click(move |_| {
                        update_values.set(vec![
                            RollbackRow {
                                id: 2,
                                label: "two-updated",
                            },
                            RollbackRow {
                                id: 1,
                                label: "panic",
                            },
                            RollbackRow {
                                id: 3,
                                label: "three-updated",
                            },
                        ])
                    });
                context.mount_unit(update, handler.view())
            }
        })
        .expect("rollback keyed view should mount");

    let initial_rows = list_elements(&raw_host);
    query_element(&raw_host, "#error-reorder")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("error update should dispatch");
    flush_browser_tasks().await;
    let error_rows = list_elements(&raw_host);
    assert!(
        errors.get() > 0,
        "updater error should reach the error handler"
    );
    assert_eq!(
        error_rows
            .iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["one", "two", "three"],
        "failed keyed error changed DOM: {}",
        raw_host.inner_html()
    );
    assert!(error_rows[0].is_same_node(Some(&initial_rows[0])));
    assert!(error_rows[1].is_same_node(Some(&initial_rows[1])));
    assert!(error_rows[2].is_same_node(Some(&initial_rows[2])));

    query_element(&raw_host, "#panic-reorder")
        .dispatch_event(&MouseEvent::new("click").expect("click should be creatable"))
        .expect("panic update should dispatch");
    flush_browser_tasks().await;

    let rows = list_elements(&raw_host);
    assert!(
        errors.get() > 0,
        "updater panic should reach the error handler"
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.text_content().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["one", "two", "three"],
        "failed keyed update changed DOM: {}",
        raw_host.inner_html()
    );
    assert!(rows[0].is_same_node(Some(&initial_rows[0])));
    assert!(rows[1].is_same_node(Some(&initial_rows[1])));
    assert!(rows[2].is_same_node(Some(&initial_rows[2])));

    mounted.dispose().expect("dispose should succeed");
    assert_eq!(raw_host.child_element_count(), 0);
    remove_host(&raw_host);
}
