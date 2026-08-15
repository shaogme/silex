use silex_i18n::{I18nBuilder, I18nStore, Runtime};

fn escaped() -> I18nStore<'static> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler = scope.error_handler(|_| {}).expect("error handler");
        I18nBuilder::new(
            scope,
            handler.view(),
        )
            .build()
            .expect("valid store")
    })
    .expect("child scope")
}

fn main() {
    let _ = escaped();
}
