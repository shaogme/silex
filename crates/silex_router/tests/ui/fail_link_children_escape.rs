use silex_core::Runtime;
use silex_router::Link;

fn main() {
    let mut runtime = Runtime::new();
    let view = runtime.child(|scope| {
        let (text, _) = scope.signal(String::from("child"));
        Link("/").children(text)
    });
    let _ = view;
}
