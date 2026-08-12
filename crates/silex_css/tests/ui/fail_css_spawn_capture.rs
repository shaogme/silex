use silex_core::Runtime;
use silex_css::prelude::*;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope
            .signal(String::from("red"))
            .expect("signal should initialize");
        let style = Style::new()
            .with_error_handler(scope.error_handler(|_| {}).expect("handler should register"))
            .raw("--color", value)
            .expect("style should build");
        require_static(style);
    });
}
