use silex_core::{ErrorReporter, Runtime};
use silex_persist::Persistent;

fn escaped() -> Persistent<'static, i32> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        Persistent::builder(scope, "counter", ErrorReporter::new(|_| {}))
            .local()
            .parse::<i32>()
            .default(0)
            .build()
    })
}

fn main() {
    let _ = escaped();
}
