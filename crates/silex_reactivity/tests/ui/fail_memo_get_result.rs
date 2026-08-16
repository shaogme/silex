use silex_reactivity::{ErrorHandlerToken, OwnerAccess, Runtime};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|scope| {
        let memo = scope
            .memo(|_| Ok::<i32, ()>(1), handler(scope))
            .expect("memo creation");
        let _: i32 = memo.get();
    });
}
