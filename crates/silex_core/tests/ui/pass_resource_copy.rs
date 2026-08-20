use silex_core::Resource;

fn assert_copy<T: Copy>() {}

fn duplicate<T: Copy + Clone>(value: T) -> (T, T, T) {
    let first = value;
    let second = value;
    let third = first.clone();
    (first, second, third)
}

fn main() {
    assert_copy::<Resource<'static, String, String>>();
    let _duplicate = duplicate::<Resource<'static, String, String>>;
}
