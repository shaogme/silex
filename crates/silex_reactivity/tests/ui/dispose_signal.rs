use silex_reactivity::{scope, signal};

fn main() {
    let id = signal::create(1i32);
    scope::dispose(id);
}
