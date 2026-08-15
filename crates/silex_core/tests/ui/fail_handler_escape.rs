use silex_core::{ErrorHandlerToken, Runtime, Scope};

fn make_handler<'scope>(
    scope: Scope<'scope>,
    value: &'scope str,
) -> ErrorHandlerToken<'static> {
    scope.error_handler(move |_| {
        assert_eq!(value, "scoped");
    }).expect("handler should register")
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let value = String::from("scoped");
        let _ = make_handler(scope, value.as_str());
    });
}
