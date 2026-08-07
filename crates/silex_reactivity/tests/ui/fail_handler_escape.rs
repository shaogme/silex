use silex_reactivity::ErrorHandler;

fn main() {
    let local = String::from("scoped");
    let value = &local;
    let _: ErrorHandler<'static, ()> = ErrorHandler::new(move |_| {
        assert_eq!(value.as_str(), "scoped");
    });
}
