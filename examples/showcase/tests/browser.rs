#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::reexports::wasm_bindgen::{JsCast, JsValue};
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlInputElement, Node};
use silex_showcase::mount_showcase_into;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn set_path(path: &str) {
    web_sys::window()
        .expect("window is available")
        .history()
        .expect("history is available")
        .replace_state_with_url(&JsValue::NULL, "", Some(path))
        .expect("test path can be set");
}

fn target() -> DomElement {
    let target = document()
        .create_element("div")
        .expect("target can be created");
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

fn stability_slider(target: &DomElement) -> HtmlInputElement {
    target
        .query_selector("input[type='range']")
        .expect("stability slider query should succeed")
        .expect("stability slider should exist")
        .dyn_into::<HtmlInputElement>()
        .expect("stability slider should be an HTML input")
}

fn adaptive_status(target: &DomElement) -> String {
    let divs = target
        .query_selector_all("div")
        .expect("status bar query should succeed");
    for index in 0..divs.length() {
        let text = divs
            .item(index)
            .expect("status bar candidate should exist")
            .text_content()
            .unwrap_or_default();
        if text.starts_with("System: ") {
            return text;
        }
    }
    panic!("adaptive status bar was not found");
}

#[wasm_bindgen_test]
fn flow_route_mounts_with_render_only_for_rows() {
    set_path("/flow");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");

    let text = target.text_content().unwrap_or_default();
    assert!(text.contains("List Rendering with Error Handling"));
    assert!(text.contains("Index For Loop Demo"));
    assert!(
        host.is_active()
            .expect("showcase should report active state")
    );

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}

#[wasm_bindgen_test]
async fn adaptive_read_formats_normalized_stability_as_percentage() {
    set_path("/advanced/adaptive");
    let target = target();
    let mut host = mount_showcase_into(target.clone().into()).expect("showcase should mount");
    let slider = stability_slider(&target);

    for (value, expected) in [("0.50", "Stability: 50%"), ("0.51", "Stability: 51%")] {
        slider.set_value(value);
        slider
            .dispatch_event(&web_sys::Event::new("input").expect("input event can be created"))
            .expect("input event should dispatch");
        flush_browser_tasks().await;

        let text = adaptive_status(&target);
        assert!(
            text.contains(expected),
            "adaptive status should contain {expected:?}, got {text:?}"
        );
    }

    host.unmount().expect("showcase should unmount");
    assert!(target.first_child().is_none());
    detach(&target.into());
    set_path("/");
}
