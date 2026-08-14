#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::bootstrap::{AppHost, AppHostError, HostState};
use silex::dom::{CleanupSink, MountContext, element::Element};
use silex::reexports::wasm_bindgen::{JsCast, JsValue};
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlElement, Node};
use silex::{Runtime, SilexError, SilexErrorKind, SilexResult};
use silex_counter::{mount_counter, mount_counter_into};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn target(id: &str) -> DomElement {
    let target = document()
        .create_element("div")
        .expect("target can be created");
    target.set_id(id);
    document()
        .body()
        .expect("body is available")
        .append_child(&target)
        .expect("target can be appended");
    target
}

fn detach(target: &Node) {
    if let Some(parent) = target.parent_node() {
        parent.remove_child(target).expect("target can be detached");
    }
}

fn reset_path() {
    web_sys::window()
        .expect("window is available")
        .history()
        .expect("history is available")
        .replace_state_with_url(&JsValue::NULL, "", Some("/"))
        .expect("test path can be reset");
}

async fn flush_browser_tasks() {
    for _ in 0..4 {
        TimeoutFuture::new(0).await;
    }
}

fn app_text(target: &DomElement) -> String {
    target.text_content().unwrap_or_default()
}

fn find_button(target: &DomElement, label: &str) -> HtmlElement {
    let buttons = target
        .query_selector_all("button")
        .expect("button query should succeed");
    for index in 0..buttons.length() {
        let button = buttons
            .item(index)
            .expect("button should exist")
            .dyn_into::<HtmlElement>()
            .expect("button should be an HTML element");
        if button.inner_text() == label {
            return button;
        }
    }
    panic!("button {label:?} was not found");
}

fn find_link(target: &DomElement, label: &str) -> HtmlElement {
    let links = target
        .query_selector_all("a")
        .expect("link query should succeed");
    for index in 0..links.length() {
        let link = links
            .item(index)
            .expect("link should exist")
            .dyn_into::<HtmlElement>()
            .expect("link should be an HTML element");
        if link.inner_text() == label {
            return link;
        }
    }
    panic!("link {label:?} was not found");
}

fn mount_text<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let handler = ctx.scope().error_handler(|_: SilexError| {})?;
    ctx.mount(Element::with_child("section", "counter-test"), handler)
}

#[wasm_bindgen_test(async)]
async fn counter_survives_entry_return_and_supports_interaction_and_teardown() {
    reset_path();
    let app = target("app");
    let mut host = mount_counter().expect("counter should mount through #app");

    assert!(host.is_active());
    assert_eq!(host.state(), "active");
    let initial = app_text(&app);
    for expected in [
        "Silex: Next Gen",
        "Explicit Counter",
        "Local State (Resets on Nav)",
        "Control Flow",
        "Suspense (Async Loading)",
    ] {
        assert!(
            initial.contains(expected),
            "missing {expected:?} in {initial:?}"
        );
    }

    find_button(&app, "+").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("1"));

    let input = app
        .query_selector("input")
        .expect("input query should succeed")
        .expect("counter input should exist")
        .dyn_into::<web_sys::HtmlInputElement>()
        .expect("counter input should be an HTML input");
    input.set_value("Ada");
    input
        .dispatch_event(&web_sys::Event::new("input").expect("input event can be created"))
        .expect("input event should dispatch");
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Hello, Ada!"));

    TimeoutFuture::new(2_100).await;
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Loaded Data from Server!"));

    find_link(&app, "About").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("This is the About Page"));

    find_link(&app, "Home").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Silex: Next Gen"));

    host.unmount().expect("counter should unmount explicitly");
    assert!(!host.is_active());
    assert_eq!(host.state(), "ready");
    assert_eq!(app.child_nodes().length(), 0);
    host.unmount()
        .expect("repeated JS-facing unmount should be idempotent");
    detach(&app.into());
}

#[wasm_bindgen_test]
fn counter_owner_unmounts_after_target_is_removed() {
    let app = target("counter-detached");
    let mut host = mount_counter_into(app.clone().into()).expect("counter should mount");
    assert!(host.is_active());

    detach(&app.clone().into());
    host.unmount()
        .expect("unmount should work after external target removal");
    assert_eq!(app.child_nodes().length(), 0);
}

#[wasm_bindgen_test]
fn counter_mount_failure_preserves_error_and_ready_state() {
    let app = target("counter-failure");
    let sink = CleanupSink::new(|report| assert!(report.is_clean()));
    let mut host = AppHost::new(app.clone().into(), sink);

    let error = host
        .mount(Runtime::new(), |_ctx| {
            Err(SilexError::recoverable(SilexErrorKind::Framework(
                "counter mount rejected".to_string(),
            )))
        })
        .expect_err("the rejected builder error should be returned");
    assert!(matches!(error, AppHostError::Mount(_)));
    assert_eq!(host.state(), HostState::Ready);
    assert!(!host.is_active());
    assert_eq!(app.child_nodes().length(), 0);

    host.mount(Runtime::new(), mount_text)
        .expect("clean rollback should leave the host reusable");
    host.unmount().expect("reused host should unmount");
    detach(&app.into());
}
