use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|scope| scope.memo(|_| Ok::<i32, ()>(1)));
}
