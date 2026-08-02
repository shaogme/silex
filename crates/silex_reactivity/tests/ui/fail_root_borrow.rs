use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let local = String::from("root-local");
        let local_ref = &local;
        scope.on_cleanup(move || assert_eq!(local_ref, "root-local"));
    });
}
