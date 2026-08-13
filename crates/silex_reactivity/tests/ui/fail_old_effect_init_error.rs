use silex_reactivity::EffectInitError;

fn main() {
    let _ = std::mem::size_of::<EffectInitError<()>>();
}
