use silex_reactivity::{ErrorHandlerToken, Runtime, Scope};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| scope.memo(|_| 1_i32, handler(scope)));
}
