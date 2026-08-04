use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let signal = root.scope().signal(0i32).0;
    drop(root);
    let _ = signal.get();
}
