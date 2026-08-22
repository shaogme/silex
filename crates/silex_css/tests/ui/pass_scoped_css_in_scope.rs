use silex_core::{Runtime, SilexContext};
use silex_css::{CssPart, DynamicCss, IntoCssReactive};
use silex_css::prelude::Style;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let value = owner
            .signal(String::from("red"))
            .expect("signal should initialize");
        let error_handler = owner
            .error_handler(|_| {})
            .expect("handler should register");
        let _style = Style::new(SilexContext::new(owner, error_handler.view()))
            .raw("--color", value)
            .expect("style should build");
        let _dynamic = DynamicCss::new("scoped").with_rule(
            &[CssPart::Lit(".scoped{"), CssPart::Val(0), CssPart::Lit("}")],
            vec![value.into_css_reactive()],
        );
    })
    .expect("transient owner should run");
}
