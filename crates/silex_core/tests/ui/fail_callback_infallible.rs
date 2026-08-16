use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let _callback = owner.callback(|_: i32| {});
    });
}
