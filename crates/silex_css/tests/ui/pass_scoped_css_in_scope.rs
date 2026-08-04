use silex_core::Runtime;
use silex_css::{CssPart, DynamicCss, IntoCssReactive};
use silex_css::prelude::Style;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(String::from("red"));
        let _style = Style::new().raw("--color", value);
        let _dynamic = DynamicCss::new("scoped").with_rule(
            &[CssPart::Lit(".scoped{"), CssPart::Val(0), CssPart::Lit("}")],
            vec![value.into_css_reactive()],
        );
    });
}
