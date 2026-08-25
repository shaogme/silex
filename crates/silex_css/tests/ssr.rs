use silex_core::{Runtime, SilexContext};
use silex_css::CssPart;
use silex_css::prelude::{
    DynamicCss, IntoCssReactive, Style, ThemeToCss, ThemeType, theme_variables,
};
use silex_dom::{
    adapters::ssr::{SerializeOptions, SsrDom},
    lifecycle::CleanupSink,
};
use silex_view::attribute::{AttributeBuilder, GlobalAttributes};
use silex_view::{Element, MountedApp};

#[derive(Clone)]
struct Theme {
    color: String,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "--theme-color:{};", self.color)
    }
}

impl ThemeType for Theme {}

impl ThemeToCss for Theme {
    fn get_variable_values(&self) -> Vec<String> {
        vec![self.color.clone()]
    }

    fn get_variable_names() -> &'static [&'static str] {
        &["--theme-color"]
    }
}

fn app(dom: &SsrDom) -> MountedApp {
    MountedApp::new(
        Runtime::new(),
        dom.context(),
        dom.document().expect("SSR document").node().clone(),
        CleanupSink::new(|_| {}),
    )
}

#[test]
fn style_serializes_through_view_attribute_pipeline() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let style = Style::new(SilexContext::new(context.access(), handler.view()))
                .raw("--test-color", "red")
                .expect("style should build");
            context.mount_unit(Element::with_child("div", "styled").style(style), handler)
        })
        .expect("mount should succeed");
    let html = dom
        .serialize(SerializeOptions::default())
        .expect("serialization should succeed");
    assert!(html.contains("class=\"slx-"));
    mounted.dispose().expect("dispose should succeed");
    assert_eq!(
        dom.serialize(Default::default()).expect("serialization"),
        ""
    );
}

#[test]
fn theme_variables_use_the_same_backend_neutral_path() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let theme = context
                .access()
                .stored(Theme {
                    color: String::from("red"),
                })
                .expect("theme should initialize");
            context.mount_unit(
                Element::with_child("div", "themed").apply(theme_variables(theme)),
                handler,
            )
        })
        .expect("mount should succeed");
    let html = dom.serialize(Default::default()).expect("serialization");
    assert!(html.contains("--theme-color:red"));
    mounted.dispose().expect("dispose should succeed");
}

#[test]
fn dynamic_css_replaces_selector_class_without_web_runtime() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let value = context
                .access()
                .signal(String::from("red"))
                .expect("signal should initialize");
            let dynamic = DynamicCss::new("slx-ssr").with_rule(
                &[
                    CssPart::Lit("."),
                    CssPart::Class,
                    CssPart::Lit(" "),
                    CssPart::SelectorVal(0),
                    CssPart::Lit("{color:red}"),
                ],
                vec![value.into_css_reactive()],
            );
            context.mount_unit(
                Element::with_child("div", "dynamic").apply(dynamic),
                handler,
            )
        })
        .expect("mount should succeed");
    let html = dom.serialize(Default::default()).expect("serialization");
    assert!(html.contains("slx-ssr"));
}
