use silex_reactivity::{callback, store};

fn main() {
    let id = store::create(());
    let _ = callback::invoke(id, Box::new(()));
}
