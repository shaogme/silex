use silex_reactivity::Callback;

fn require_send<T: Send>() {}

fn main() {
    require_send::<Callback<'static, ()>>();
}
