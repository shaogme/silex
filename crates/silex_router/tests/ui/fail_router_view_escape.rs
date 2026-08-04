use silex_core::Runtime;
use silex_router::Router;

fn main() {
    let mut runtime = Runtime::new();
    let view = runtime.child(|scope| Router(scope));
    let _ = view;
}
