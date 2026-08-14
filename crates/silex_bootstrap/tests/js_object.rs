#![cfg(all(target_arch = "wasm32", feature = "js-object"))]

use silex_bootstrap::{AppHost, JsAppHost};
use silex_core::{Runtime, SilexError, SilexResult};
use silex_dom::{CleanupSink, MountContext, element::Element};
use wasm_bindgen_test::*;
use web_sys::Node;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn target() -> Node {
    let target: Node = document()
        .create_element("div")
        .expect("target can be created")
        .into();
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

fn clean_sink() -> CleanupSink {
    CleanupSink::new(|report| assert!(report.is_clean()))
}

fn mount_text<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let handler = ctx.scope().error_handler(|_: SilexError| {})?;
    ctx.mount(Element::with_child("section", "js-owner"), handler)
}

#[wasm_bindgen_test]
fn js_wrapper_retains_the_mounted_owner_and_unmount_is_idempotent() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());
    host.mount(Runtime::new(), mount_text)
        .expect("application should mount before JS transfer");

    let mut js_host = JsAppHost::from_app_host(host);
    assert!(js_host.is_active());
    assert_eq!(js_host.state(), "active");
    assert_eq!(target.text_content().as_deref(), Some("js-owner"));

    js_host.unmount().expect("first JS unmount should succeed");
    assert!(!js_host.is_active());
    assert_eq!(js_host.state(), "ready");
    assert_eq!(target.child_nodes().length(), 0);

    js_host
        .unmount()
        .expect("repeated JS unmount should remain idempotent");
    detach(&target);
}
