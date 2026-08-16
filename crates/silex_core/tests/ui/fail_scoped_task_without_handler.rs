use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        owner.spawn_scoped(async {});
    });
}
