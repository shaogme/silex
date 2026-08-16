use silex_i18n::{I18nBuilder, Runtime, Rx, t};

fn escaped() -> Rx<'static, String> {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let handler = owner.error_handler(|_| {}).expect("error handler");
            let store = I18nBuilder::new(owner, handler.view())
                .build()
                .expect("valid store");
            t!(store, "missing.key").expect("translation")
        })
        .expect("transient owner")
}

fn main() {
    let _ = escaped();
}
