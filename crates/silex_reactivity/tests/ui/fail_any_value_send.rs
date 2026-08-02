use silex_reactivity::AnyValue;

fn require_send<T: Send>() {}

fn main() {
    require_send::<AnyValue<'static>>();
}
