#![cfg(target_arch = "wasm32")]

use silex_core::{ErrorHandlerToken, ErrorReporter, OwnerAccess, Runtime, SilexResult};
use silex_dom::{
    adapters::browser::BrowserDom,
    model::{DomNode, ElementSpec},
    runtime::DomContext,
};
use silex_html::{a, svg_a};
use silex_view::{MountContext, MountInstance, MountOwnerToken, View};
use wasm_bindgen_test::*;
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

fn browser() -> BrowserDom {
    BrowserDom::from_window().expect("browser DOM should be available")
}

fn host(dom: &DomContext) -> (DomNode, Element) {
    let body = dom
        .document_body()
        .expect("browser document body lookup should succeed")
        .expect("browser document should have a body");
    let host = dom
        .create_element(ElementSpec::new("div"))
        .expect("browser test host should be created");
    dom.append(body.node(), host.node())
        .expect("browser test host should attach");
    let raw_host = web_sys::window()
        .expect("browser window should be available")
        .document()
        .expect("browser document should be available")
        .body()
        .and_then(|body| body.last_element_child())
        .expect("raw host should be visible");
    (host.node().clone(), raw_host)
}

fn error_handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("browser test error handler should register")
}

fn mount_view<'owner, V: View<'owner>>(
    view: &V,
    owner: &MountOwnerToken<'owner>,
    dom: &DomContext,
    parent: &DomNode,
    error_handler: ErrorReporter<'owner>,
) -> SilexResult<MountInstance<'owner>> {
    let context =
        MountContext::for_parent(dom.clone(), parent.clone(), owner.clone(), error_handler);
    let instance = context.mount(view)?;
    context.transaction().commit()?;
    Ok(instance)
}

#[wasm_bindgen_test]
fn html_anchor_uses_html_metadata_and_mounts() {
    let browser = browser();
    let dom = browser.context();
    let (host, raw_host) = host(&dom);
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let handler = error_handler(owner);
            let mount_owner = MountOwnerToken::new(owner);
            let view = a("Documentation");
            let _ = mount_view(&view, &mount_owner, &dom, &host, handler.view())
                .expect("HTML anchor should mount");
            let element = raw_host
                .first_element_child()
                .expect("HTML anchor should be present");
            assert_eq!(element.tag_name(), "A");
            assert_eq!(
                element.namespace_uri().as_deref(),
                Some("http://www.w3.org/1999/xhtml")
            );
        })
        .expect("transient HTML anchor owner should initialize");
}

#[wasm_bindgen_test]
fn svg_anchor_uses_svg_namespace_metadata_and_mounts() {
    let browser = browser();
    let dom = browser.context();
    let (host, raw_host) = host(&dom);
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let handler = error_handler(owner);
            let mount_owner = MountOwnerToken::new(owner);
            let view = svg_a("Documentation");
            let _ = mount_view(&view, &mount_owner, &dom, &host, handler.view())
                .expect("SVG anchor should mount");
            let element = raw_host
                .first_element_child()
                .expect("SVG anchor should be present");
            assert_eq!(element.tag_name(), "a");
            assert_eq!(
                element.namespace_uri().as_deref(),
                Some("http://www.w3.org/2000/svg")
            );
        })
        .expect("transient SVG anchor owner should initialize");
}
