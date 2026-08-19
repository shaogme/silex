#![cfg(target_arch = "wasm32")]

use std::borrow::Cow;

use silex_core::{
    ErrorHandlerToken, ErrorReporter, OwnerAccess, Runtime, SilexError, SilexErrorKind, SilexResult,
};
use silex_dom::attribute::AttributeBuilder;
use silex_dom::element::Element;
use silex_dom::view::{AnyView, ApplyAttributes, MountInstance, MountOwner, MountOwnerToken, View};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{Element as DomElement, Node};

silex_dom::define_tag!(
    TestSvg,
    web_sys::SvgElement,
    "svg",
    test_svg,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

fn host() -> DomElement {
    web_sys::window()
        .expect("window is available in browser tests")
        .document()
        .expect("document is available in browser tests")
        .create_element("div")
        .expect("test host can be created")
}

fn mounted(host: &DomElement) -> DomElement {
    host.first_element_child()
        .expect("reactive element should be mounted")
}

#[wasm_bindgen_test]
fn svg_with_children_preserves_svg_namespace_and_case_sensitive_attributes() {
    let host = host();
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let error_handler = test_handler(owner);
            let owner = MountOwnerToken::new(owner);
            let view = test_svg("icon").attr("viewBox", "0 0 24 24");
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler.view())
                .expect("svg view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.namespace_uri().as_deref(),
                Some("http://www.w3.org/2000/svg")
            );
            assert!(element.dyn_ref::<web_sys::SvgElement>().is_some());
            assert_eq!(
                element.get_attribute("viewBox").as_deref(),
                Some("0 0 24 24")
            );
            assert!(element.get_attribute("viewbox").is_none());
        })
        .expect("child owner should initialize");
}

fn style_text(element: &DomElement) -> String {
    element.get_attribute("style").unwrap_or_default()
}

struct RejectingView {
    element: Rc<RefCell<Option<DomElement>>>,
}

impl<'owner> ApplyAttributes<'owner> for RejectingView {}

impl<'owner> View<'owner> for RejectingView {
    fn mount(
        &self,
        owner: &dyn MountOwner<'owner>,
        parent: &Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'owner>>,
        error_handler: ErrorReporter<'owner>,
    ) -> SilexResult<MountInstance<'owner>> {
        let document = web_sys::window()
            .ok_or_else(|| SilexError::fatal(SilexErrorKind::Dom("window is unavailable".into())))?
            .document()
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom("document is unavailable".into()))
            })?;
        let element = document.create_element("div").map_err(SilexError::fatal)?;
        parent.append_child(&element).map_err(SilexError::fatal)?;
        let token = owner.token();
        for attr in attrs {
            attr.apply(&element, &token, error_handler)?;
        }
        *self.element.borrow_mut() = Some(element);
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "intentional mount rejection".to_string(),
        )))
    }
}

#[wasm_bindgen_test]
fn reactive_static_str_attribute_updates() {
    let host = host();
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (read, write) = owner.signal("initial").expect("signal should initialize");
            let error_handler = test_handler(owner);
            let owner = MountOwnerToken::new(owner);
            let view = Element::new("button").attr("data-state", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler.view())
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("initial")
            );

            write.set("updated").expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("updated")
            );
        })
        .expect("child owner should initialize");
}

#[wasm_bindgen_test]
fn reactive_borrowed_str_attribute_updates() {
    let host = host();
    let initial = String::from("initial");
    let updated = String::from("updated");
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (read, write) = owner
                .signal(initial.as_str())
                .expect("signal should initialize");
            let error_handler = test_handler(owner);
            let owner = MountOwnerToken::new(owner);
            let view = Element::new("div").attr("data-value", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler.view())
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-value").as_deref(),
                Some("initial")
            );

            write
                .set(updated.as_str())
                .expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-value").as_deref(),
                Some("updated")
            );
        })
        .expect("child owner should initialize");
}

#[wasm_bindgen_test]
fn reactive_cow_attribute_updates() {
    let host = host();
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (read, write) = owner
                .signal(Cow::Borrowed("initial"))
                .expect("signal should initialize");
            let error_handler = test_handler(owner);
            let owner = MountOwnerToken::new(owner);
            let view = Element::new("span").attr("data-state", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler.view())
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("initial")
            );

            write
                .set(Cow::Owned(String::from("updated")))
                .expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-state").as_deref(),
                Some("updated")
            );
        })
        .expect("child owner should initialize");
}

#[wasm_bindgen_test]
fn reactive_string_reference_attribute_updates() {
    let host = host();
    let initial = String::from("initial");
    let updated = String::from("updated");
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (read, write) = owner.signal(&initial).expect("signal should initialize");
            let error_handler = test_handler(owner);
            let owner = MountOwnerToken::new(owner);
            let view = Element::new("p").attr("data-text", read.into_rx());
            let _ = view
                .mount(&owner, &host, Vec::new(), error_handler.view())
                .expect("reactive view should mount");

            let element = mounted(&host);
            assert_eq!(
                element.get_attribute("data-text").as_deref(),
                Some("initial")
            );

            write.set(&updated).expect("signal should be writable");
            assert_eq!(
                element.get_attribute("data-text").as_deref(),
                Some("updated")
            );
        })
        .expect("child owner should initialize");
}

#[wasm_bindgen_test]
fn reactive_str_classes_merge_update_and_cleanup() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element;
    {
        let owner = root.access();
        let (read, write) = owner
            .signal("dynamic-one")
            .expect("signal should initialize");
        let error_handler = test_handler(owner);
        let owner = MountOwnerToken::new(owner);
        let view = Element::new("div")
            .attr("class", "static")
            .attr("class", read.into_rx());
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler.view())
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(element.class_list().contains("static"));
        assert!(element.class_list().contains("dynamic-one"));

        write.set("dynamic-two").expect("signal should be writable");
        assert!(element.class_list().contains("static"));
        assert!(!element.class_list().contains("dynamic-one"));
        assert!(element.class_list().contains("dynamic-two"));
    }

    root.close().expect("root cleanup should succeed");
    assert!(element.class_list().contains("static"));
    assert!(!element.class_list().contains("dynamic-two"));
}

#[wasm_bindgen_test]
fn reactive_str_stylesheet_merges_update_and_cleanup() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element;
    {
        let owner = root.access();
        let (read, write) = owner
            .signal("color: red;")
            .expect("signal should initialize");
        let error_handler = test_handler(owner);
        let owner = MountOwnerToken::new(owner);
        let view = Element::new("div")
            .attr("style", "display: block;")
            .attr("style", read.into_rx());
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler.view())
            .expect("reactive view should mount");

        element = mounted(&host);
        let initial = style_text(&element);
        assert!(initial.contains("display: block"), "{initial}");
        assert!(initial.contains("color: red"), "{initial}");

        write
            .set("color: blue;")
            .expect("signal should be writable");
        let updated = style_text(&element);
        assert!(updated.contains("display: block"), "{updated}");
        assert!(updated.contains("color: blue"), "{updated}");
        assert!(!updated.contains("color: red"), "{updated}");
    }

    root.close().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_cow_style_property_updates_and_cleans_up() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element;
    {
        let owner = root.access();
        let (read, write) = owner
            .signal(Cow::Borrowed("red"))
            .expect("signal should initialize");
        let error_handler = test_handler(owner);
        let owner = MountOwnerToken::new(owner);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("display", "block"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler.view())
            .expect("reactive view should mount");

        element = mounted(&host);
        let initial = style_text(&element);
        assert!(initial.contains("display: block"), "{initial}");
        assert!(initial.contains("color: red"), "{initial}");

        write
            .set(Cow::Owned(String::from("blue")))
            .expect("signal should be writable");
        let updated = style_text(&element);
        assert!(updated.contains("display: block"), "{updated}");
        assert!(updated.contains("color: blue"), "{updated}");
        assert!(!updated.contains("color: red"), "{updated}");
    }

    root.close().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_borrowed_str_style_property_updates_and_cleans_up() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element;
    {
        let owner = root.access();
        let (read, write) = owner.signal("red").expect("signal should initialize");
        let error_handler = test_handler(owner);
        let owner = MountOwnerToken::new(owner);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("display", "block"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler.view())
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(style_text(&element).contains("color: red"));
        write.set("blue").expect("signal should be writable");
        assert!(style_text(&element).contains("color: blue"));
    }

    root.close().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_string_reference_style_property_updates_and_cleans_up() {
    let host = host();
    let initial = String::from("red");
    let updated = String::from("blue");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element;
    {
        let owner = root.access();
        let (read, write) = owner.signal(&initial).expect("signal should initialize");
        let error_handler = test_handler(owner);
        let owner = MountOwnerToken::new(owner);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("display", "block"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler.view())
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(style_text(&element).contains("color: red"));
        write.set(&updated).expect("signal should be writable");
        assert!(style_text(&element).contains("color: blue"));
    }

    root.close().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("display: block"), "{cleaned}");
    assert!(!cleaned.contains("color"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_style_property_restores_static_value_after_dispose() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    let element;
    {
        let owner = root.access();
        let (read, write) = owner
            .signal(String::from("red"))
            .expect("signal should initialize");
        let error_handler = test_handler(owner);
        let owner = MountOwnerToken::new(owner);
        let view = Element::new("div")
            .attr("style", ("color", read.into_rx()))
            .attr("style", ("color", "green"));
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler.view())
            .expect("reactive view should mount");

        element = mounted(&host);
        assert!(style_text(&element).contains("color: red"));
        write
            .set(String::from("blue"))
            .expect("signal should be writable");
        assert!(style_text(&element).contains("color: blue"));
    }

    root.close().expect("root cleanup should succeed");
    let cleaned = style_text(&element);
    assert!(cleaned.contains("color: green"), "{cleaned}");
}

#[wasm_bindgen_test]
fn reactive_style_plan_cleans_up_after_failed_mount() {
    let host = host();
    let captured = Rc::new(RefCell::new(None));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let (read, _) = owner
                .signal(String::from("red"))
                .expect("signal should initialize");
            let error_handler = test_handler(owner);
            let owner = MountOwnerToken::new(owner);
            let view = AnyView::new(RejectingView {
                element: captured.clone(),
            })
            .attr("style", ("color", read.into_rx()));
            assert!(
                view.mount(&owner, &host, Vec::new(), error_handler.view())
                    .is_err()
            );
        })
        .expect("child owner should initialize");

    let element = captured
        .borrow()
        .clone()
        .expect("failed mount should expose its detached element");
    assert!(!style_text(&element).contains("color"));
}
