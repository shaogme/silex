use silex_reactivity::ErrorHandlerToken;

fn require_send<T: Send>() {}

fn main() {
    require_send::<ErrorHandlerToken<'static, ()>>();
}
