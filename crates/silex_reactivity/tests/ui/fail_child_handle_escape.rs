use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _signal = runtime.child(|scope| {
        scope.child(|child| {
            child
                .signal(0i32)
                .expect("signal creation should succeed")
                .0
        })
    });
}
