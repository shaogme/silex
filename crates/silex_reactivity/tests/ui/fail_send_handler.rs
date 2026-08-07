use silex_reactivity::ErrorHandler;

fn require_send<T: Send>() {}

fn main() {
    require_send::<ErrorHandler<'static, ()>>();
}
