#![cfg(target_arch = "wasm32")]

use silex_core::{
    ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime, SilexError, SilexErrorKind,
};
use silex_dom::{
    attribute::GlobalEventAttributes,
    view::{MountOwnerToken, View},
};
use silex_html::{a, svg_a};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn host() -> Element {
    web_sys::window()
        .expect("browser window should be available")
        .document()
        .expect("browser document should be available")
        .create_element("div")
        .expect("browser test host should be created")
}

fn error_handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("browser test error handler should register")
}

fn first_node(instance: &silex_dom::view::MountInstance<'_>) -> Node {
    instance
        .first_node()
        .expect("anchor mount should produce one node")
        .clone()
}

#[wasm_bindgen_test]
fn html_anchor_mounts_as_html_anchor_element() {
    let host = host();
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let handler = error_handler(owner);
            let mount_owner = MountOwnerToken::new(owner);
            let instance = a("Documentation")
                .mount(&mount_owner, &host, Vec::new(), handler.view())
                .expect("HTML anchor should mount");
            let element = first_node(&instance)
                .dyn_into::<web_sys::HtmlAnchorElement>()
                .expect("HTML anchor should cast to HtmlAnchorElement");

            assert_eq!(element.tag_name(), "A");
            assert_eq!(
                element.namespace_uri().as_deref(),
                Some("http://www.w3.org/1999/xhtml")
            );
        })
        .expect("transient HTML anchor owner should initialize");
}

#[wasm_bindgen_test]
fn svg_anchor_mounts_as_svg_anchor_element() {
    let host = host();
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let handler = error_handler(owner);
            let mount_owner = MountOwnerToken::new(owner);
            let instance = svg_a("Documentation")
                .mount(&mount_owner, &host, Vec::new(), handler.view())
                .expect("SVG anchor should mount");
            let element = first_node(&instance)
                .dyn_into::<web_sys::SvgaElement>()
                .expect("SVG anchor should cast to SvgaElement");

            assert_eq!(element.tag_name(), "a");
            assert_eq!(
                element.namespace_uri().as_deref(),
                Some("http://www.w3.org/2000/svg")
            );
        })
        .expect("transient SVG anchor owner should initialize");
}

#[wasm_bindgen_test]
fn explicit_svg_anchor_node_ref_binds_and_cleans_up() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner should initialize");
    let node_ref = root
        .access()
        .node_ref::<web_sys::SvgaElement>()
        .expect("SVG anchor NodeRef should initialize");

    {
        let owner = root.access();
        let handler = error_handler(owner);
        let mount_owner = MountOwnerToken::new(owner);
        let instance = svg_a("Documentation")
            .node_ref(node_ref)
            .mount(&mount_owner, &host, Vec::new(), handler.view())
            .expect("SVG anchor with an explicit NodeRef should mount");

        assert!(
            node_ref
                .get()
                .expect("NodeRef should be readable")
                .is_some()
        );
        assert!(
            first_node(&instance)
                .dyn_ref::<web_sys::SvgaElement>()
                .is_some()
        );
    }

    root.close().expect("root cleanup should succeed");
    assert!(matches!(
        node_ref.get(),
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::NoSuchNode
        )))
    ));
    assert!(host.first_element_child().is_none());
}

#[wasm_bindgen_test]
fn wrong_svg_anchor_node_ref_type_is_reported_at_mount() {
    let host = host();
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let node_ref = owner
                .node_ref::<web_sys::HtmlAnchorElement>()
                .expect("wrong-type NodeRef should initialize");
            let handler = error_handler(owner);
            let mount_owner = MountOwnerToken::new(owner);
            let result = svg_a("Documentation").node_ref(node_ref).mount(
                &mount_owner,
                &host,
                Vec::new(),
                handler.view(),
            );

            assert!(matches!(
                result,
                Err(SilexError::Fatal(SilexErrorKind::Dom(message)))
                    if message.contains("NodeRef type mismatch")
            ));
            assert!(host.first_element_child().is_none());
        })
        .expect("transient wrong-type NodeRef owner should initialize");
}
