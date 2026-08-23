use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _transaction = runtime
        .with_transient(|owner| owner.transaction(|transaction| Ok(transaction)))
        .expect("the owner should initialize");
    let _ = _transaction;
}
