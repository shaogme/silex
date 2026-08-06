use silex_i18n::{I18nBuilder, Memo, Runtime, t};

fn escaped() -> Memo<'static, String> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let store = I18nBuilder::new(scope).build().expect("valid store");
        t!(store, "missing.key")
    })
}

fn main() {
    let _ = escaped();
}
