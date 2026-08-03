use silex_reactivity::AnyValue;

fn main() {
    let _ = std::mem::size_of::<AnyValue<'static>>();
}
