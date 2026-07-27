use silex_reactivity::{signal, store};

fn main() {
    let id = signal::create(1i32);
    let _ = store::try_with::<i32, _>(id, |value| *value);
}
