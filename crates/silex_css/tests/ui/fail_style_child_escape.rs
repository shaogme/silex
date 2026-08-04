use silex_core::Runtime;
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    let style = runtime.child(|scope| {
        let (value, _) = scope.signal(String::from("red"));
        Style::new().raw("--color", value)
    });
    let _ = style;
}
