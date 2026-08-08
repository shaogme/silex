use silex_core::{ErrorReporter, Runtime};

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let value = String::from("scoped");
        let reporter: ErrorReporter<'_> = scope.error_handler(|_| {
            let _ = &value;
        });
        require_static(reporter);
    });
}
