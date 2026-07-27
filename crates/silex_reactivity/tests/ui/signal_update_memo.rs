use silex_reactivity::{memo, signal};

fn main() {
    let id = memo::create::<i32, _>(|_| 1);
    signal::update(id, |value: &mut i32| *value += 1);
}
