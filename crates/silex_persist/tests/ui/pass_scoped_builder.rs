use silex_core::Scope;
use silex_persist::Persistent;

fn build<'scope>(scope: Scope<'scope>) -> Persistent<'scope, i32> {
    Persistent::builder(
        scope,
        "counter",
        scope
            .error_handler(|_| {})
            .expect("error handler should be registered"),
    )
    .local()
    .parse::<i32>()
    .default(0)
    .build()
    .expect("persistent binding should build")
}

fn main() {
    let _ = build;
}
