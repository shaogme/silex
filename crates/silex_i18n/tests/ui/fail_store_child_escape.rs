use silex_i18n::{I18nBuilder, I18nStore, Runtime};

fn escaped() -> I18nStore<'static> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        I18nBuilder::new(
            scope,
            scope.error_handler(|_| {}).expect("error handler"),
        )
            .build()
            .expect("valid store")
    })
    .expect("child scope")
}

fn main() {
    let _ = escaped();
}
