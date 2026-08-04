use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let local = String::from("root-local");

    {
        let scope = root.scope();
        let stored = scope.stored(&local);
        assert_eq!(stored.with(|value| value.as_str()), "root-local");
    }

    root.dispose().expect("root disposal should succeed");
}
