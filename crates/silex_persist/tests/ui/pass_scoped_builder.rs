use silex_core::Scope;
use silex_persist::Persistent;

fn build<'scope>(scope: Scope<'scope>) -> Persistent<'scope, i32> {
    Persistent::builder(scope, "counter", scope.error_handler(|_| {}))
        .local()
        .parse::<i32>()
        .default(0)
        .build()
}

fn main() {
    let _ = build;
}
