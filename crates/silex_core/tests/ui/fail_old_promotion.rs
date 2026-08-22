use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let source = owner.signal(1i32).expect("signal should initialize");
        let _ = source.into_rx(&owner);
    });
}
