use silex_i18n::{EffectHandle, RwSignal};

fn main() {
    let _ = RwSignal::new(1_u32);
    let _ = EffectHandle::new(|| {});
}
