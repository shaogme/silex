use silex_reactivity::{ErrorHandler, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = scope.effect(|| {}, ErrorHandler::new(|_| {}));
    });
}
