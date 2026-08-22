use silex_i18n::{EffectHandle, Signal};

fn main() {
    let _ = Signal::new(1_u32);
    let _ = EffectHandle::new(|| {});
}
