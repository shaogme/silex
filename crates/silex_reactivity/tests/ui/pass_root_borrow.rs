use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should initialize");
    let local = String::from("root-local");

    {
        let scope = root.access();
        let stored = scope.stored(&local).expect("stored value should initialize");
        assert_eq!(stored.with(|value| value.as_str()).expect("stored read"), "root-local");
    }

    root.close().expect("root disposal should succeed");
}
