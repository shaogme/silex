use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should initialize");
    let local = String::from("root-local");

    {
        let scope = root.scope();
        let stored = scope.stored(&local).expect("stored value should initialize");
        assert_eq!(stored.with(|value| value.as_str()).expect("stored read"), "root-local");
    }

    root.dispose().expect("root disposal should succeed");
}
