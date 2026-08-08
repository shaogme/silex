use silex_core::{ErrorHandler, Runtime, Scope, SilexError};

fn make_handler<'scope>(
    scope: Scope<'scope>,
    value: &'scope str,
) -> ErrorHandler<'static, SilexError> {
    scope.error_handler(move |_| {
        assert_eq!(value, "scoped");
    })
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let value = String::from("scoped");
        let _ = make_handler(scope, value.as_str());
    });
}
