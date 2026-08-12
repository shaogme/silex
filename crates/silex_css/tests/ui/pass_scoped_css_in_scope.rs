use silex_core::Runtime;
use silex_css::{CssPart, DynamicCss, IntoCssReactive};
use silex_css::prelude::Style;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope
            .signal(String::from("red"))
            .expect("signal should initialize");
        let _style = Style::new()
            .with_error_handler(scope.error_handler(|_| {}).expect("handler should register"))
            .raw("--color", value)
            .expect("style should build");
        let _dynamic = DynamicCss::new("scoped").with_rule(
            &[CssPart::Lit(".scoped{"), CssPart::Val(0), CssPart::Lit("}")],
            vec![value.into_css_reactive()],
        );
    })
    .expect("child scope should run");
}
