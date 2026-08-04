use silex_core::Runtime;
use silex_css::prelude::*;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(String::from("red"));
        let style = Style::new().raw("--color", value);
        require_static(style);
    });
}
