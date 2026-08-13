use silex_reactivity::{ErrorHandler, Runtime, Scope};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let memo = scope
            .memo(|_| Ok::<i32, ()>(1), handler(scope))
            .expect("memo creation");
        let _: i32 = memo.get();
    });
}
