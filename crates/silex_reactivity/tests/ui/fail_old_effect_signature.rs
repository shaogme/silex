use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.effect(|| {}, scope.error_handler(|_| {}));
    });
}
