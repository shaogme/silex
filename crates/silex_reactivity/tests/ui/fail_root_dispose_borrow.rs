use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root creation should succeed");
    let (read, _) = root
        .scope()
        .signal(1i32)
        .expect("signal creation should succeed");
    root.dispose().expect("root disposal should succeed");
    let _ = read.get();
}
