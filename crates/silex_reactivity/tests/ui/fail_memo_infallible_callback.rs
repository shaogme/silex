use silex_reactivity::{ErrorHandlerToken, OwnerAccess, Runtime};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|scope| scope.memo(|_| 1_i32, handler(scope)));
}
