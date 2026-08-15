use silex_core::{ErrorHandlerToken, ErrorReporter, Runtime};

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let value = String::from("scoped");
        let token: ErrorHandlerToken<'_> = scope
            .error_handler(|_| {
                let _ = &value;
            })
            .expect("handler should register");
        let reporter: ErrorReporter<'_> = token.view();
        require_static(reporter);
    });
}
