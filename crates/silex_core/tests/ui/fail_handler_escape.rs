use silex_core::{ErrorHandler, SilexError};

fn make_handler(value: &str) -> ErrorHandler<'static, SilexError> {
    ErrorHandler::new(move |_| {
        assert_eq!(value, "scoped");
    })
}

fn main() {
    let value = String::from("scoped");
    let _ = make_handler(value.as_str());
}
