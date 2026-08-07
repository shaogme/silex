use silex_core::{ErrorHandler, SilexError};

fn require_send<T: Send>() {}

fn main() {
    require_send::<ErrorHandler<'static, SilexError>>();
}
