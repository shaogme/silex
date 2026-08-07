use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        scope.spawn_scoped(async {});
    });
}
