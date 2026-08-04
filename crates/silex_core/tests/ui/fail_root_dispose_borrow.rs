use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let (read, _) = root.scope().signal(1i32);
    root.dispose().expect("root disposal should succeed");
    let _ = read.get();
}
