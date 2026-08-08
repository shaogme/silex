#![cfg(all(target_arch = "wasm32", feature = "browser-bootstrap"))]

use silex_bootstrap::{
    BootstrapError, BrowserBootstrap, JsAppHost, LifecycleReporter, PageLifecyclePolicy,
};
use silex_core::{Runtime, SilexError, SilexResult};
use silex_dom::{MountContext, element::Element};
use std::rc::Rc;
use wasm_bindgen_test::*;
use web_sys::{Element as DomElement, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn element(id: &str) -> DomElement {
    let element = document()
        .create_element("div")
        .expect("target can be created");
    element.set_id(id);
    document()
        .body()
        .expect("body is available")
        .append_child(&element)
        .expect("target can be appended");
    element
}

fn detach(target: &Node) {
    if let Some(parent) = target.parent_node() {
        parent.remove_child(target).expect("target can be detached");
    }
}

fn mount_text<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let handler = context.scope().error_handler(|_: SilexError| {});
    context.mount(Element::with_child("section", "browser-owner"), handler)
}

fn reporter() -> LifecycleReporter {
    Rc::new(|_| {})
}

#[wasm_bindgen_test]
fn from_id_resolves_target_and_delegates_mount() {
    let target = element("phase-four-target");
    let mut bootstrap = BrowserBootstrap::from_id("phase-four-target")
        .expect("browser bootstrap should resolve an existing id");

    bootstrap
        .mount(Runtime::new(), mount_text)
        .expect("browser bootstrap should mount");
    assert!(bootstrap.is_active());
    assert_eq!(target.text_content().as_deref(), Some("browser-owner"));

    bootstrap
        .unmount()
        .expect("browser bootstrap should unmount");
    assert_eq!(target.child_nodes().length(), 0);
    detach(&target.into());
}

#[wasm_bindgen_test]
fn missing_id_is_reported_without_a_partial_controller() {
    let error = match BrowserBootstrap::from_id("missing-phase-four-target") {
        Ok(_) => panic!("missing target must not create a controller"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        BootstrapError::TargetNotFound(id) if id == "missing-phase-four-target"
    ));
}

#[wasm_bindgen_test]
fn removing_page_lifecycle_allows_manual_js_owner_transfer() {
    let target = element("phase-four-transfer");
    let mut bootstrap = BrowserBootstrap::from_element(target.clone());
    bootstrap
        .mount(Runtime::new(), mount_text)
        .expect("browser bootstrap should mount");
    bootstrap
        .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter())
        .expect("page lifecycle should install");
    bootstrap.remove_page_lifecycle();

    let mut js_host: JsAppHost = bootstrap
        .into_js_host()
        .expect("manual controller should transfer to JS owner");
    assert!(js_host.is_active());
    js_host.unmount().expect("JS owner should unmount");
    detach(&target.into());
}

#[wasm_bindgen_test]
fn non_manual_policy_cannot_transfer_listener_ownership_implicitly() {
    let target = element("phase-four-policy");
    let mut bootstrap = BrowserBootstrap::from_element(target.clone());
    bootstrap
        .mount(Runtime::new(), mount_text)
        .expect("browser bootstrap should mount");
    bootstrap
        .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter())
        .expect("page lifecycle should install");

    let error = match bootstrap.into_js_host() {
        Ok(_) => panic!("non-manual controller must not transfer its listener"),
        Err(error) => error,
    };
    assert!(matches!(error, BootstrapError::Lifecycle(message) if message.contains("Manual")));
    assert_eq!(target.child_nodes().length(), 0);
    detach(&target.into());
}
