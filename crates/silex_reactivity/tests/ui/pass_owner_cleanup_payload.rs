use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    let owner = root.access();
    let handler = owner
        .error_handler(|_: ()| {})
        .expect("cleanup handler");

    assert!(owner
        .on_owner_cleanup(
            String::from("generic payload"),
            |payload| {
                assert_eq!(payload, "generic payload");
                Ok::<(), ()>(())
            },
            handler.view(),
        )
        .is_ok());
    root.close().expect("root close");
}
