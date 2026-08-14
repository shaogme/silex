#![cfg(target_arch = "wasm32")]

extern crate silex_macros_test as silex;

use js_sys::{Array, Reflect};
use silex::core::{ErrorReporter, Runtime, Scope, SilexContext, SilexResult};
use silex::css::types::{Hex, hex, px};
use silex::dom::attribute::{AttributeBuilder, GlobalAttributes};
use silex::dom::prelude::AnyView;
use silex::dom::view::{MountOwner, ScopedMountOwner, View};
use silex::macros::{classes, css, global, styled, tw};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope.error_handler(|_| {}).unwrap()
}

fn test_owner<'scope>(scope: Scope<'scope>) -> (ScopedMountOwner<'scope>, ErrorReporter<'scope>) {
    let error_handler = test_handler(scope);
    (ScopedMountOwner::new(scope), error_handler)
}

global! {
    pub MacroGlobal<'scope>(
        error_handler: ErrorReporter<'scope>,
        color: silex::core::reactivity::Signal<'scope, Hex>,
        selector: silex::core::reactivity::Signal<'scope, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}

global! {
    pub MacroForeignGlobal<'scope>(
        error_handler: ErrorReporter<'scope>,
        color: silex::core::reactivity::Signal<'scope, Hex>,
    ) {
        :root { --macro-foreign-global: $(color); }
    }
}

global! {
    pub MacroMixedForeignGlobal<'scope>(
        error_handler: ErrorReporter<'scope>,
        color: silex::core::reactivity::Signal<'scope, Hex>,
        selector: silex::core::reactivity::Signal<'scope, String>,
    ) {
        :root { --macro-mixed-foreign-global: $(color); }
        $selector { color: red; }
    }
}

styled! {
    pub MacroStyledValue<'scope><div>(
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
        color: silex::core::reactivity::Signal<'scope, Hex>,
    ) {
        color: $(color);
    }
}

styled! {
    pub MacroStyledSelector<'scope><div>(
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
        selector: silex::core::reactivity::Signal<'scope, String>,
    ) {
        & $selector { color: red; }
    }
}

styled! {
    pub MacroStyledVariant<'scope><div>(
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
        selector: silex::core::reactivity::Signal<'scope, String>,
    ) {
        variants: {
            mode: {
                light: { & $selector { color: rgb(17, 34, 51); } },
                dark: { & $selector { color: rgb(68, 85, 102); } },
            }
        }
    }
}

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("browser tests have a window")
        .document()
        .expect("browser tests have a document")
}

fn document_style_contains_all(needles: &[&str]) -> bool {
    let sheets = Reflect::get(
        document().as_ref(),
        &JsValue::from_str("adoptedStyleSheets"),
    )
    .ok();
    if let Some(sheets) = sheets {
        let sheets = Array::from(&sheets);
        for sheet_index in 0..sheets.length() {
            let rules = Reflect::get(&sheets.get(sheet_index), &JsValue::from_str("cssRules"))
                .ok()
                .map(|rules| Array::from(&rules));
            if let Some(rules) = rules {
                for rule_index in 0..rules.length() {
                    let text = Reflect::get(&rules.get(rule_index), &JsValue::from_str("cssText"))
                        .ok()
                        .and_then(|value| value.as_string())
                        .unwrap_or_default();
                    if needles.iter().all(|needle| text.contains(needle)) {
                        return true;
                    }
                }
            }
        }
    }

    let styles = document().get_elements_by_tag_name("style");
    for index in 0..styles.length() {
        if styles
            .item(index)
            .and_then(|style| style.text_content())
            .is_some_and(|text| needles.iter().all(|needle| text.contains(needle)))
        {
            return true;
        }
    }
    false
}

fn document_style_contains(needle: &str) -> bool {
    document_style_contains_all(&[needle])
}

async fn flush_style_microtasks() {
    for _ in 0..4 {
        JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED))
            .await
            .expect("microtask promise resolves");
    }
}

fn style_text(element: &web_sys::Element) -> String {
    element.get_attribute("style").unwrap_or_default()
}

fn remove_host(host: &web_sys::Element) {
    let host_node: web_sys::Node = host.clone().into();
    host_node
        .parent_node()
        .expect("test host has a parent")
        .remove_child(&host_node)
        .expect("test host can be removed");
}

fn mount_foreign_css<'scope>(
    local_root: &'scope silex::core::RootHandle,
    foreign_root: &'scope silex::core::RootHandle,
    host: &web_sys::Element,
) -> SilexResult<()> {
    let local_scope = local_root.scope();
    let foreign_scope = foreign_root.scope();
    let (color, _) = foreign_scope.signal(hex("#123456")).unwrap();
    let css: SilexResult<silex::css::DynamicCss<'_>> = css!(test_handler(local_scope); {
        --macro-foreign-css: $(color);
    });
    let view = silex::html::div(()).apply(css?);
    let (owner, error_handler) = test_owner(local_scope);
    assert!(view.mount(&owner, host, Vec::new(), error_handler).is_err());
    Ok(())
}

fn mount_foreign_global<'scope>(
    local_root: &'scope silex::core::RootHandle,
    foreign_root: &'scope silex::core::RootHandle,
    host: &web_sys::Element,
) {
    let local_scope = local_root.scope();
    let foreign_scope = foreign_root.scope();
    let (color, _) = foreign_scope.signal(hex("#654321")).unwrap();
    let (owner, error_handler) = test_owner(local_scope);
    assert!(
        MacroForeignGlobal(test_handler(local_scope), color.into())
            .unwrap()
            .mount(&owner, host, Vec::new(), error_handler)
            .is_err()
    );
}

fn mount_mixed_foreign_global<'scope>(
    local_root: &'scope silex::core::RootHandle,
    foreign_root: &'scope silex::core::RootHandle,
    host: &web_sys::Element,
) {
    let local_scope = local_root.scope();
    let foreign_scope = foreign_root.scope();
    let (color, _) = local_scope.signal(hex("#112233")).unwrap();
    let (selector, _) = foreign_scope
        .signal(String::from("macro-mixed-foreign-selector"))
        .unwrap();
    let (owner, error_handler) = test_owner(local_scope);
    assert!(
        MacroMixedForeignGlobal(test_handler(local_scope), color.into(), selector.into())
            .unwrap()
            .mount(&owner, host, Vec::new(), error_handler)
            .is_err()
    );
}

#[wasm_bindgen_test]
fn foreign_macro_inputs_fail_before_dom_or_stylesheet_side_effects() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut local_runtime = Runtime::new();
    let local_root = local_runtime
        .run()
        .expect("local runtime root can be created");
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime
        .run()
        .expect("foreign runtime root can be created");

    mount_foreign_css(&local_root, &foreign_root, &host)
        .expect("foreign CSS macro should reject the input");
    assert_eq!(host.child_element_count(), 0);
    assert!(!document_style_contains("macro-foreign-css"));

    mount_foreign_global(&local_root, &foreign_root, &host);
    assert_eq!(host.child_element_count(), 0);
    assert!(!document_style_contains("macro-foreign-global"));

    mount_mixed_foreign_global(&local_root, &foreign_root, &host);
    assert_eq!(host.child_element_count(), 0);
    assert!(!document_style_contains("macro-mixed-foreign-global"));
    assert!(!document_style_contains("macro-mixed-foreign-selector"));

    local_root
        .dispose()
        .expect("local foreign-input owner can be disposed");
    foreign_root
        .dispose()
        .expect("foreign source owner can be disposed");
    remove_host(&host);
}

#[wasm_bindgen_test]
fn css_dynamic_value_mounts_updates_and_cleans_with_owner() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    {
        let scope = root.scope();
        let (width, set_width) = scope.signal(px(4)).unwrap();
        let view = (|| -> SilexResult<_> {
            let css: SilexResult<silex::css::DynamicCss<'_>> =
                css!(test_handler(scope); width: $(width););
            Ok(silex::html::div(()).apply(css?))
        })()
        .expect("dynamic CSS macro should expand");
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        element = host
            .last_element_child()
            .expect("dynamic css view mounts an element");
        assert!(style_text(&element).contains("4px"));

        set_width.set(px(8)).unwrap();
        assert!(style_text(&element).contains("8px"));
    }

    root.dispose().expect("css owner can be disposed");
    assert!(element.class_name().is_empty());
    assert!(style_text(&element).is_empty());
    remove_host(&host);
}

#[wasm_bindgen_test(async)]
async fn css_dynamic_selector_updates_and_detaches_on_owner_dispose() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    let first_dynamic_class;
    {
        let scope = root.scope();
        let (selector, set_selector) = scope.signal(String::from("macro-css-selector-a")).unwrap();
        let view = (|| -> SilexResult<_> {
            let css: SilexResult<silex::css::DynamicCss<'_>> = css!(test_handler(scope); {
                & $selector { color: red; }
            });
            Ok(silex::html::div(()).apply(css?))
        })()
        .expect("dynamic CSS selector macro should expand");
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        element = host
            .last_element_child()
            .expect("dynamic css selector view mounts an element");
        first_dynamic_class = element
            .class_name()
            .split_whitespace()
            .find(|token| token.contains("-d"))
            .expect("dynamic css selector class is present")
            .to_string();

        flush_style_microtasks().await;
        assert!(document_style_contains_all(&[
            "@layer utilities",
            "macro-css-selector-a",
        ]));

        set_selector
            .set(String::from("macro-css-selector-b"))
            .unwrap();
        flush_style_microtasks().await;
        let second_class = element.class_name();
        assert!(!second_class.contains(&first_dynamic_class));
        assert!(!document_style_contains("macro-css-selector-a"));
        assert!(document_style_contains_all(&[
            "@layer utilities",
            "macro-css-selector-b",
        ]));
    }

    root.dispose().expect("css selector owner can be disposed");
    flush_style_microtasks().await;
    assert!(element.class_name().is_empty());
    assert!(!document_style_contains("macro-css-selector-b"));
    remove_host(&host);
}

#[wasm_bindgen_test(async)]
async fn css_dynamic_selector_dispose_before_pending_style_flush_does_not_readd_sheet() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    {
        let scope = root.scope();
        let (selector, _) = scope
            .signal(String::from("macro-pending-dispose-selector"))
            .unwrap();
        let view = (|| -> SilexResult<_> {
            let css: SilexResult<silex::css::DynamicCss<'_>> = css!(test_handler(scope); {
                $selector { color: red; }
            });
            Ok(silex::html::div(()).apply(css?))
        })()
        .expect("dynamic CSS selector macro should expand");
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        element = host
            .last_element_child()
            .expect("dynamic selector view mounts an element");
        assert!(!element.class_name().is_empty());
    }

    root.dispose()
        .expect("pending stylesheet owner can be disposed");
    flush_style_microtasks().await;
    assert!(element.class_name().is_empty());
    assert!(!document_style_contains("macro-pending-dispose-selector"));
    remove_host(&host);
}

#[wasm_bindgen_test(async)]
async fn css_dynamic_selector_stylesheet_is_leased_across_owners() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut first_runtime = Runtime::new();
    let first_root = first_runtime
        .run()
        .expect("first runtime root can be created");
    let mut second_runtime = Runtime::new();
    let second_root = second_runtime
        .run()
        .expect("second runtime root can be created");
    let first_element;
    let second_element;
    {
        let scope = first_root.scope();
        let (selector, _) = scope.signal(String::from("macro-shared-selector")).unwrap();
        let view = (|| -> SilexResult<_> {
            let css: SilexResult<silex::css::DynamicCss<'_>> = css!(test_handler(scope); {
                $selector { color: red; }
            });
            Ok(silex::html::div(()).apply(css?))
        })()
        .expect("dynamic CSS selector macro should expand");
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        first_element = host
            .last_element_child()
            .expect("first shared selector view mounts an element");
    }
    {
        let scope = second_root.scope();
        let (selector, _) = scope.signal(String::from("macro-shared-selector")).unwrap();
        let view = (|| -> SilexResult<_> {
            let css: SilexResult<silex::css::DynamicCss<'_>> = css!(test_handler(scope); {
                $selector { color: red; }
            });
            Ok(silex::html::div(()).apply(css?))
        })()
        .expect("dynamic CSS selector macro should expand");
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        second_element = host
            .last_element_child()
            .expect("second shared selector view mounts an element");
    }

    flush_style_microtasks().await;
    assert!(document_style_contains("macro-shared-selector"));
    assert!(!first_element.class_name().is_empty());
    assert!(!second_element.class_name().is_empty());

    first_root
        .dispose()
        .expect("first shared stylesheet owner can be disposed");
    flush_style_microtasks().await;
    assert!(!second_element.class_name().is_empty());
    assert!(document_style_contains("macro-shared-selector"));

    second_root
        .dispose()
        .expect("second shared stylesheet owner can be disposed");
    flush_style_microtasks().await;
    assert!(!document_style_contains("macro-shared-selector"));
    remove_host(&host);
}

#[wasm_bindgen_test]
fn conditional_tw_switches_one_owner_bound_class_and_cleans_on_dispose() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    let first_class;
    {
        let scope = root.scope();
        let (condition, set_condition) = scope.signal(true).unwrap();
        let view = silex::html::div(()).apply(tw!(
            "inline-flex",
            (
                condition,
                "bg-blue-500 text-white",
                "bg-slate-500 text-black"
            )
        ));
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        element = host
            .last_element_child()
            .expect("conditional tw view mounts an element");
        first_class = element.class_name();
        assert!(!first_class.is_empty());

        set_condition.set(false).unwrap();
        let second_class = element.class_name();
        assert_ne!(first_class, second_class);
        assert!(
            first_class
                .split_whitespace()
                .any(|token| !second_class.split_whitespace().any(|next| next == token)),
            "old conditional class remains: {first_class} -> {second_class}"
        );
    }

    root.dispose()
        .expect("conditional tw owner can be disposed");
    assert!(element.class_name().is_empty());
    remove_host(&host);
}

#[wasm_bindgen_test]
fn classes_reactive_toggle_updates_and_cleans_without_removing_static_classes() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    {
        let scope = root.scope();
        let (active, set_active) = scope.signal(true).unwrap();
        let (dynamic_classes, set_dynamic_classes) = scope
            .signal(String::from("macro-owned macro-reactive"))
            .unwrap();
        let view = silex::html::div(()).apply(classes![
            "macro-static",
            "macro-active" => active,
            "macro-static" => active,
            "macro-owned" => active,
            dynamic_classes,
        ]);
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");
        element = host
            .last_element_child()
            .expect("classes view mounts an element");
        assert!(element.class_list().contains("macro-static"));
        assert!(element.class_list().contains("macro-active"));
        assert!(element.class_list().contains("macro-owned"));
        assert!(element.class_list().contains("macro-reactive"));

        set_active.set(false).unwrap();
        assert!(element.class_list().contains("macro-static"));
        assert!(!element.class_list().contains("macro-active"));
        assert!(element.class_list().contains("macro-owned"));
        assert!(element.class_list().contains("macro-reactive"));

        set_dynamic_classes
            .set(String::from("macro-owned"))
            .unwrap();
        assert!(element.class_list().contains("macro-owned"));
        assert!(!element.class_list().contains("macro-reactive"));

        set_dynamic_classes.set(String::new()).unwrap();
        assert!(!element.class_list().contains("macro-owned"));
        assert!(element.class_list().contains("macro-static"));

        set_active.set(true).unwrap();
        assert!(element.class_list().contains("macro-static"));
        assert!(element.class_list().contains("macro-active"));
        assert!(element.class_list().contains("macro-owned"));

        set_dynamic_classes
            .set(String::from("macro-reactive"))
            .unwrap();
        assert!(element.class_list().contains("macro-owned"));
        assert!(element.class_list().contains("macro-reactive"));

        set_active.set(false).unwrap();
        assert!(!element.class_list().contains("macro-owned"));
        assert!(element.class_list().contains("macro-reactive"));

        set_dynamic_classes.set(String::new()).unwrap();
        assert!(element.class_list().contains("macro-static"));
        assert!(!element.class_list().contains("macro-reactive"));
    }

    root.dispose().expect("classes owner can be disposed");
    assert!(element.class_list().contains("macro-static"));
    assert!(!element.class_list().contains("macro-active"));
    assert!(!element.class_list().contains("macro-owned"));
    remove_host(&host);
}

#[wasm_bindgen_test]
fn static_class_strings_are_applied_as_separate_dom_tokens() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    {
        let scope = root.scope();
        let view = silex::html::div(()).class("static-first static-second");
        let (owner, error_handler) = test_owner(scope);
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("macro view should mount");

        let element = host
            .last_element_child()
            .expect("static class view mounts an element");
        assert!(element.class_list().contains("static-first"));
        assert!(element.class_list().contains("static-second"));
    }

    root.dispose().expect("static class owner can be disposed");
    remove_host(&host);
}

#[wasm_bindgen_test]
fn styled_dynamic_value_cleans_inline_property_on_owner_dispose() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    {
        let scope = root.scope();
        let (color, set_color) = scope.signal(hex("#123456")).unwrap();
        let (owner, error_handler) = test_owner(scope);
        let ctx = SilexContext::new(scope, error_handler);
        let view = MacroStyledValue(ctx, AnyView::new(()), color).build();
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("styled value view should mount");
        element = host
            .last_element_child()
            .expect("styled value view mounts an element");
        assert!(style_text(&element).contains("#123456"));

        set_color.set(hex("#654321")).unwrap();
        assert!(style_text(&element).contains("#654321"));
    }

    root.dispose().expect("styled value owner can be disposed");
    assert!(style_text(&element).is_empty());
    remove_host(&host);
}

#[wasm_bindgen_test]
fn styled_static_descriptor_rejects_foreign_inputs_without_outer_mount_aggregation() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut local_runtime = Runtime::new();
    let local_root = local_runtime
        .run()
        .expect("local runtime root can be created");
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime
        .run()
        .expect("foreign runtime root can be created");

    let local_scope = local_root.scope();
    let foreign_scope = foreign_root.scope();
    let (color, _) = foreign_scope.signal(hex("#123456")).unwrap();
    let getter = color
        .into_rx()
        .map(|value| value.to_string(), test_handler(local_scope))
        .expect("CSS getter should initialize");
    let operation = silex::css::StyledVariantBinding::new(
        silex::css::layers::COMPONENTS,
        Vec::new(),
        Vec::new(),
    )
    .with_static_styles(
        vec![(
            "macro-standalone-styled-static",
            ".macro-standalone-styled-static { color: red; }",
        )],
        vec![getter],
    )
    .into_op();
    let (owner, error_handler) = test_owner(local_scope);

    assert!(
        operation
            .apply(&host, &owner.token(), error_handler)
            .is_err()
    );
    assert!(!document_style_contains("macro-standalone-styled-static"));

    local_root
        .dispose()
        .expect("local styled owner can be disposed");
    foreign_root
        .dispose()
        .expect("foreign styled source can be disposed");
    remove_host(&host);
}

#[wasm_bindgen_test(async)]
async fn styled_dynamic_selector_updates_and_detaches_on_owner_dispose() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    let first_class;
    {
        let scope = root.scope();
        let (selector, set_selector) = scope.signal(String::from("macro-selector-a")).unwrap();
        let (owner, error_handler) = test_owner(scope);
        let ctx = SilexContext::new(scope, error_handler);
        let view = MacroStyledSelector(ctx, AnyView::new(()), selector).build();
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("styled selector view should mount");
        element = host
            .last_element_child()
            .expect("styled selector view mounts an element");
        first_class = element.class_name();
        flush_style_microtasks().await;
        assert!(document_style_contains_all(&[
            "@layer components",
            "macro-selector-a"
        ]));

        set_selector.set(String::from("macro-selector-b")).unwrap();
        flush_style_microtasks().await;
        let second_class = element.class_name();
        assert_ne!(first_class, second_class);
        assert!(
            !second_class.contains(
                first_class
                    .split_whitespace()
                    .find(|token| token.contains("-d"))
                    .expect("dynamic selector class is present")
            )
        );
        assert!(!document_style_contains("macro-selector-a"));
        assert!(document_style_contains_all(&[
            "@layer components",
            "macro-selector-b"
        ]));
    }

    root.dispose()
        .expect("styled selector owner can be disposed");
    flush_style_microtasks().await;
    assert!(!document_style_contains("macro-selector-b"));
    remove_host(&host);
}

#[wasm_bindgen_test(async)]
async fn styled_dynamic_variant_switches_rules_and_cleans_on_dispose() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let element;
    {
        let scope = root.scope();
        let (mode, set_mode) = scope.signal(String::from("light")).unwrap();
        let (selector, _) = scope
            .signal(String::from("macro-variant-selector"))
            .unwrap();
        let (owner, error_handler) = test_owner(scope);
        let ctx = SilexContext::new(scope, error_handler);
        let view = MacroStyledVariant(ctx, AnyView::new(()), selector)
            .mode(mode)
            .expect("styled variant mode should be valid")
            .build();
        let _ = view
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("styled variant view should mount");
        element = host
            .last_element_child()
            .expect("styled variant view mounts an element");
        flush_style_microtasks().await;
        assert!(document_style_contains_all(&[
            "@layer components",
            "rgb(17, 34, 51)"
        ]));

        set_mode.set(String::from("dark")).unwrap();
        flush_style_microtasks().await;
        assert!(document_style_contains_all(&[
            "@layer components",
            "rgb(68, 85, 102)"
        ]));
        assert!(!document_style_contains("rgb(17, 34, 51)"));
    }

    root.dispose()
        .expect("styled variant owner can be disposed");
    flush_style_microtasks().await;
    assert!(!document_style_contains("rgb(68, 85, 102)"));
    remove_host(&host);
    let _ = element;
}

#[wasm_bindgen_test(async)]
async fn dynamic_global_mounts_without_a_dom_node_and_cleans_on_dispose() {
    let host = document()
        .create_element("div")
        .expect("test host can be created");
    document()
        .body()
        .expect("document has a body")
        .append_child(&host)
        .expect("test host can be mounted");

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root can be created");
    let mut second_runtime = Runtime::new();
    let second_root = second_runtime
        .run()
        .expect("second runtime root can be created");
    {
        let set_color = root.with_scope(|scope| {
            let (color, set_color) = scope.signal(hex("#123456")).unwrap();
            let (selector, _) = scope.signal(String::from(".macro-target")).unwrap();
            let (owner, error_handler) = test_owner(scope);
            let _ = MacroGlobal(test_handler(scope), color.into(), selector.into())
                .unwrap()
                .mount(&owner, &host, Vec::new(), error_handler)
                .expect("global macro view should mount");
            assert_eq!(host.child_element_count(), 0);
            set_color
        });
        flush_style_microtasks().await;
        // Firefox CSSOM serializes the original hex value as an rgb() color.
        assert!(document_style_contains_all(&[
            "@layer base",
            "rgb(18, 52, 86)"
        ]));
        assert!(document_style_contains_all(&[
            "@layer base",
            "macro-target"
        ]));

        set_color.set(hex("#654321")).unwrap();
        flush_style_microtasks().await;
        assert!(document_style_contains_all(&[
            "@layer base",
            "rgb(101, 67, 33)"
        ]));
    }
    second_root.with_scope(|scope| {
        let (color, _) = scope.signal(hex("#abcdef")).unwrap();
        let (selector, _) = scope
            .signal(String::from(".macro-target-secondary"))
            .unwrap();
        let (owner, error_handler) = test_owner(scope);
        let _ = MacroGlobal(test_handler(scope), color.into(), selector.into())
            .unwrap()
            .mount(&owner, &host, Vec::new(), error_handler)
            .expect("global macro view should mount");
    });
    flush_style_microtasks().await;
    assert!(document_style_contains_all(&[
        "@layer base",
        "rgb(171, 205, 239)"
    ]));
    assert!(document_style_contains_all(&[
        "@layer base",
        "macro-target-secondary"
    ]));

    root.dispose().expect("global owner can be disposed");
    flush_style_microtasks().await;
    assert!(!document_style_contains("rgb(18, 52, 86)"));
    assert!(!document_style_contains("rgb(101, 67, 33)"));
    assert!(document_style_contains("rgb(171, 205, 239)"));

    second_root
        .dispose()
        .expect("second global owner can be disposed");
    flush_style_microtasks().await;
    assert!(!document_style_contains("rgb(171, 205, 239)"));

    let host_node: web_sys::Node = host.into();
    host_node
        .parent_node()
        .expect("test host has a parent")
        .remove_child(&host_node)
        .expect("test host can be removed");
}
