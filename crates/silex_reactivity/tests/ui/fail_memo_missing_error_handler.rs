use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| scope.memo(|_| Ok::<i32, ()>(1)));
}
