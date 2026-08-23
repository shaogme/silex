use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _future = runtime.with_transient(|owner| {
        owner.transaction(|transaction| {
            Ok(async move {
                core::future::pending::<()>().await;
                let _ = transaction.phase();
            })
        })
    });
}
