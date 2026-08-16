use silex_i18n::{I18nBuilder, I18nStore, Runtime};

fn escaped() -> I18nStore<'static> {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let handler = owner.error_handler(|_| {}).expect("error handler");
            I18nBuilder::new(owner, handler.view())
                .build()
                .expect("valid store")
        })
        .expect("transient owner")
}

fn main() {
    let _ = escaped();
}
