use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(1_i32);
        scope.spawn_scoped(async move {
            let _ = value.get();
        });
    });
}
