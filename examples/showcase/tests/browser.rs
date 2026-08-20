#![cfg(target_arch = "wasm32")]

use silex::reexports::wasm_bindgen::JsValue;
use silex::reexports::web_sys::{self, Document, Element as DomElement, Node};
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
