#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex::reexports::wasm_bindgen::{JsCast, JsValue};
use silex::reexports::web_sys::{self, Document, Element as DomElement, HtmlElement, Node};
use silex_router_example::{mount_router, mount_router_into};
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

#[wasm_bindgen_test(async)]
async fn router_resolves_typed_params_and_unmounts_cleanly() {
    reset_path();
    let app = target("app");
    let mut host = mount_router().expect("router should mount through #app");

    assert!(app_text(&app).contains("Home Page"));

    find_link(&app, "Users").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Select a User:"));

    find_link(&app, "👤 Silex Expert (ID: 42)").click();
    flush_browser_tasks().await;
    let detail = app_text(&app);
    assert!(
        detail.contains("User Profile: #42"),
        "unexpected detail: {detail}"
    );
    assert!(detail.contains("Current Path: /users/42"));

    find_link(&app, "Search").click();
    flush_browser_tasks().await;
    assert!(app_text(&app).contains("Search Query Test"));

    host.unmount().expect("router should unmount explicitly");
    assert_eq!(app.child_nodes().length(), 0);
    host.unmount()
        .expect("repeated JS-facing unmount should be idempotent");
    detach(&app.into());
}

#[wasm_bindgen_test]
fn router_owner_unmounts_after_target_is_removed() {
    reset_path();
    let app = target("router-detached");
    let mut host = mount_router_into(app.clone().into()).expect("router should mount");

    detach(&app.clone().into());
    host.unmount()
        .expect("unmount should work after external target removal");
    assert_eq!(app.child_nodes().length(), 0);
}
