use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root creation should succeed");
    let (read, _) = root
        .access()
        .signal(1i32)
        .expect("signal creation should succeed");
    drop(root);
    let _ = read.get();
}
