use silex_core::ErrorReporter;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let value = String::from("scoped");
    let reporter = ErrorReporter::new(|_| {
        let _ = &value;
    });
    require_static(reporter);
}
