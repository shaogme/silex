#![cfg(all(target_arch = "wasm32", feature = "test-style-fallback"))]

use js_sys::Promise;
use silex_core::Runtime;
use silex_css::{CssPart, DynamicCss, IntoCssReactive, prelude::inject_style};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom},
    view::{ScopedViewOwner, ViewOwner},
};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{Document, Element, HtmlStyleElement, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> Document {
    web_sys::window()
        .expect("browser tests have a window")
        .document()
        .expect("browser tests have a document")
}

fn style_text_containing(needle: &str) -> Option<String> {
    let head = document().head()?;
    let children = head.children();
    for index in 0..children.length() {
        let element = children.item(index)?;
        if element.tag_name() != "STYLE" {
            continue;
        }
        let style = element.dyn_into::<HtmlStyleElement>().ok()?;
        let text = style.text_content().unwrap_or_default();
        if text.contains(needle) {
            return Some(text);
        }
    }
    None
}

async fn flush_style_microtasks() {
    for _ in 0..4 {
        JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
            .await
            .expect("microtask promise resolves");
    }
}

fn mount_point() -> Element {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("browser tests have a body")
        .append_child(&host)
        .expect("test host can be mounted");
    host
}

fn remove(node: &Node) {
    if let Some(parent) = node.parent_node() {
        parent.remove_child(node).expect("test node can be removed");
    }
}

#[wasm_bindgen_test(async)]
async fn style_tag_fallback_injects_updates_and_detaches_on_owner_dispose() {
    const STATIC_MARKER: &str = "slx-fallback-static-marker";
    inject_style(
        "slx-fallback-static",
        ".slx-fallback-static-marker{color:red}",
    );
    flush_style_microtasks().await;
    assert!(style_text_containing(STATIC_MARKER).is_some());

    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope);
        let token = owner.token();
        let dynamic = DynamicCss::new("slx-fallback-dynamic").with_rule(
            &[
                CssPart::Lit("."),
                CssPart::Class,
                CssPart::Lit(" "),
                CssPart::SelectorVal(0),
                CssPart::Lit("{color:red}"),
            ],
            vec![value.into_css_reactive()],
        );
        dynamic.apply(&element, ApplyTarget::Class, &token);
        let initial = style_text_containing("slx-fallback-dynamic").expect("fallback style exists");
        assert!(initial.contains("red"), "{initial}");

        set_value.set(String::from("blue"));
        let updated = style_text_containing("slx-fallback-dynamic").expect("fallback style exists");
        assert!(updated.contains("blue"), "{updated}");
    });

    flush_style_microtasks().await;
    assert!(style_text_containing("slx-fallback-dynamic").is_none());
    assert!(style_text_containing(STATIC_MARKER).is_some());
    remove(&host.into());
}
