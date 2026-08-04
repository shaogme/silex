use silex_core::Runtime;
use silex_css::{CssPart, DynamicCss, IntoCssReactive};

fn main() {
    let mut runtime = Runtime::new();
    let css = runtime.child(|scope| {
        let (value, _) = scope.signal(String::from("red"));
        DynamicCss::new("child").with_rule(
            &[CssPart::Lit(".child{"), CssPart::Val(0), CssPart::Lit("}")],
            vec![value.into_css_reactive()],
        )
    });
    let _ = css;
}
