use silex_i18n::{I18nBuilder, I18nStore, Runtime};

fn escaped() -> I18nStore<'static> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| I18nBuilder::new(scope).build().expect("valid store"))
}

fn main() {
    let _ = escaped();
}
