use silex_reactivity::{effect, signal};

fn main() {
    let id = effect::create(|| {});
    let _ = signal::try_get::<()>(id);
}
