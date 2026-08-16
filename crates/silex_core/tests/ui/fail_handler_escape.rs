use silex_core::{ErrorHandlerToken, Runtime, OwnerAccess};

fn make_handler<'owner>(
    owner: OwnerAccess<'owner>,
    value: &'owner str,
) -> ErrorHandlerToken<'static> {
    owner.error_handler(move |_| {
        assert_eq!(value, "scoped");
    }).expect("handler should register")
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let value = String::from("scoped");
        let _ = make_handler(owner, value.as_str());
    });
}
