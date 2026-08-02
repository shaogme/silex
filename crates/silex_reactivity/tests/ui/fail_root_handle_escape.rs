use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _signal = runtime.run(|scope| scope.signal(0i32).0);
}
