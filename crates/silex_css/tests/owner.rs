#![cfg(target_arch = "wasm32")]

use silex_core::Runtime;
use silex_css::{
    CssPart, DynamicCss, IntoCssReactive,
    prelude::{Style, ThemeToCss, ThemeType, theme_variables},
};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom},
    view::{ScopedViewOwner, ViewOwner},
};
use std::fmt::{Display, Formatter};
use wasm_bindgen_test::*;
use web_sys::{Element, Node};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("browser tests have a window")
        .document()
        .expect("browser tests have a document")
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

#[derive(Clone)]
struct TestTheme {
    color: String,
}

impl Display for TestTheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "--theme-color:{};", self.color)
    }
}

impl ThemeType for TestTheme {}

impl ThemeToCss for TestTheme {
    fn get_variable_values(&self) -> Vec<String> {
        vec![self.color.clone()]
    }

    fn get_variable_names() -> &'static [&'static str] {
        &["--theme-color"]
    }
}

#[wasm_bindgen_test]
fn style_updates_inline_values_and_cleans_on_scope_dispose() {
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
        let class_name = Style::new()
            .raw("--test-color", value)
            .apply_to_element(&element, &token);

        assert!(element.class_list().contains(&class_name));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("red")
        );

        set_value.set(String::from("blue"));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("blue")
        );
    });

    assert!(element.class_name().is_empty());
    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn theme_updates_variables_and_cleans_on_scope_dispose() {
    let host = mount_point();
    let element = document()
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (theme, set_theme) = scope.signal(TestTheme {
            color: String::from("red"),
        });
        let owner = ScopedViewOwner::new(scope);
        let token = owner.token();
        theme_variables(theme).apply(&element, ApplyTarget::Apply, &token);

        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("--theme-color: red")
        );
        set_theme.set(TestTheme {
            color: String::from("blue"),
        });
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("--theme-color: blue")
        );
    });

    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn svg_style_updates_inline_values_and_cleans_on_scope_dispose() {
    let host = mount_point();
    let element = document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
        .expect("svg element can be created");
    host.append_child(&element)
        .expect("svg element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope);
        let token = owner.token();
        Style::new()
            .raw("--svg-color", value)
            .apply_to_element(&element, &token);
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("red")
        );

        set_value.set(String::from("blue"));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("blue")
        );
    });

    assert!(
        element
            .get_attribute("style")
            .unwrap_or_default()
            .is_empty()
    );
    remove(&host.into());
}

#[wasm_bindgen_test]
fn dynamic_css_replaces_rule_class_and_cleans_on_scope_dispose() {
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
        let dynamic = DynamicCss::new("slx-owner-test").with_rule(
            &[
                CssPart::Lit("."),
                CssPart::Class,
                CssPart::Lit("{color:"),
                CssPart::Val(0),
                CssPart::Lit("}"),
            ],
            vec![value.into_css_reactive()],
        );

        dynamic.apply(&element, ApplyTarget::Class, &token);
        let first_class = element.class_name();
        assert!(first_class.contains("slx-owner-test"));
        assert!(first_class.contains("-d"));

        set_value.set(String::from("blue"));
        let second_class = element.class_name();
        assert!(second_class.contains("slx-owner-test"));
        assert_ne!(first_class, second_class);
    });

    assert!(element.class_name().is_empty());
    remove(&host.into());
}
