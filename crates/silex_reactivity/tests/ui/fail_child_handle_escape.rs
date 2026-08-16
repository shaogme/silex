use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _signal = runtime.with_transient(|scope| {
        scope.with_transient(|child| {
            child
                .signal(0i32)
                .expect("signal creation should succeed")
                .0
        })
    });
}
