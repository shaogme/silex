use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let (source, _) = owner.signal(1i32);
        let _ = source.into_rx(&owner);
    });
}
