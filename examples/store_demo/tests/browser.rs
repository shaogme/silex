#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::reexports::wasm_bindgen::JsCast;
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlElement, Node};
use silex_store_demo::mount_store_into;
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
async fn store_demo_mounts_updates_fields_and_disposes_root() {
    let app = target("store-app");
    let mut host = mount_store_into(app.clone().into()).expect("Store demo should mount");

    assert!(
        host.is_active()
            .expect("store demo should report active state")
    );
    assert!(app_text(&app).contains("Alice"));
    assert!(app_text(&app).contains("25"));

    find_button(&app, "Increment Age").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("26"));

    let inputs = app
        .query_selector_all("input")
        .expect("input query should succeed");
    let name_input = inputs
        .item(0)
        .expect("name input should exist")
        .dyn_into::<web_sys::HtmlInputElement>()
        .expect("name input should be an HTML input");
    name_input.set_value("Ada");
    name_input
        .dispatch_event(&web_sys::Event::new("input").expect("input event can be created"))
        .expect("input event should dispatch");
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Ada"));

    host.unmount().expect("Store demo should unmount");
    assert!(
        !host
            .is_active()
            .expect("store demo should report active state")
    );
    assert_eq!(app.child_nodes().length(), 0);
    host.unmount()
        .expect("repeated unmount should remain idempotent");
    detach(&app.into());
}
