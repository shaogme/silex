use silex_i18n::{Effect, RwSignal};

fn main() {
    let _ = RwSignal::new(1_u32);
    let _ = Effect::new(|| {});
}
