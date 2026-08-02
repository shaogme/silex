use silex_core::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _escaped = runtime.run(|scope| scope.signal(1).0);
}
