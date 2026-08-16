use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root creation should succeed");
    let signal = root
        .access()
        .signal(0i32)
        .expect("signal creation should succeed")
        .0;
    drop(root);
    let _ = signal.get();
}
