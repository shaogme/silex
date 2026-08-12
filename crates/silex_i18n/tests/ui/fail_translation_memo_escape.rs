use silex_i18n::{I18nBuilder, Runtime, Rx, t};

fn escaped() -> Rx<'static, String> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let store = I18nBuilder::new(
            scope,
            scope.error_handler(|_| {}).expect("error handler"),
        )
            .build()
            .expect("valid store");
        t!(store, "missing.key").expect("translation")
    })
    .expect("child scope")
}

fn main() {
    let _ = escaped();
}
