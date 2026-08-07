use silex_core::{ErrorReporter, Scope};
use silex_persist::Persistent;

fn build<'scope>(scope: Scope<'scope>) -> Persistent<'scope, i32> {
    Persistent::builder(scope, "counter", ErrorReporter::new(|_| {}))
        .local()
        .parse::<i32>()
        .default(0)
        .build()
}

fn main() {
    let _ = build;
}
