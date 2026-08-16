use silex_core::OwnerAccess;
use silex_persist::Persistent;

fn build<'scope>(owner: OwnerAccess<'scope>) -> Persistent<'scope, i32> {
    Persistent::builder(
        owner,
        "counter",
        owner
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
