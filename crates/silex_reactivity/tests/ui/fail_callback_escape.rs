use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _callback = runtime.child(|scope| {
        scope.child(|child| {
            child
                .callback(|_: ()| Ok::<(), ()>(()))
                .expect("callback should initialize")
        })
    });
}
