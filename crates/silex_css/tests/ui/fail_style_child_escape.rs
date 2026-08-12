use silex_core::Runtime;
use silex_css::prelude::*;

fn main() {
    let mut runtime = Runtime::new();
    let style = runtime.child(|scope| {
        let (value, _) = scope
            .signal(String::from("red"))
            .expect("signal should initialize");
        Style::new()
            .with_error_handler(scope.error_handler(|_| {}).expect("handler should register"))
            .raw("--color", value)
            .expect("style should build")
    });
    let _ = style;
}
