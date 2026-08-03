use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, _) = scope.signal(1i32);
        let _ = source.into_rx(&scope);
    });
}
