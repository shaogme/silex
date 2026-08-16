use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _callback = runtime.with_transient(|scope| {
        scope.with_transient(|child| {
            child
                .callback(|_: ()| Ok::<(), ()>(()))
                .expect("callback should initialize")
        })
    });
}
