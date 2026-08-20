#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::reexports::wasm_bindgen::JsCast;
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlElement, Node};
use silex_error_demo::mount_error_boundary_demo_into;
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

#[wasm_bindgen_test(async)]
async fn recoverable_error_renders_fallback_and_root_unmount_cleans_the_demo() {
    let app = target("error-boundary-demo-app");
    let mut host = mount_error_boundary_demo_into(app.clone().into())
        .expect("error boundary demo should mount");

    assert!(
        host.is_active()
            .expect("error boundary demo should report active state")
    );
    assert!(app_text(&app).contains("Component is running normally."));
    assert!(app_text(&app).contains("The panic component is currently hidden."));

    find_button(&app, "Trigger Result::Err").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Caught Recoverable Error!"));
    assert!(app_text(&app).contains("User clicked the error button!"));

    host.unmount().expect("demo should unmount explicitly");
    assert!(
        !host
            .is_active()
            .expect("error boundary demo should report active state")
    );
    assert_eq!(app.child_nodes().length(), 0);
    host.unmount()
        .expect("repeated unmount should remain idempotent");
    detach(&app.into());
}

#[wasm_bindgen_test(async)]
#[ignore = "requires nightly wasm build-std with panic unwind"]
async fn render_panic_reaches_fallback_and_root_unmount_cleans_the_demo() {
    let app = target("error-boundary-panic-app");
    let mut host = mount_error_boundary_demo_into(app.clone().into())
        .expect("error boundary demo should mount");

    find_button(&app, "Show Panic Component").click();
    flush_browser_tasks().await;
    find_button(&app, "Click to Panic Immediately").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Caught Panic!"));
    assert!(app_text(&app).contains("KA-BOOM! Panic in render function."));

    host.unmount().expect("demo should unmount explicitly");
    assert!(
        !host
            .is_active()
            .expect("error boundary demo should report active state")
    );
    assert_eq!(app.child_nodes().length(), 0);
    host.unmount()
        .expect("repeated unmount should remain idempotent");
    detach(&app.into());
}
