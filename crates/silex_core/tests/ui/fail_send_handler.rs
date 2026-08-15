use silex_core::ErrorHandler;

fn require_send<T: Send>() {}

fn main() {
    require_send::<ErrorHandler<'static>>();
}
