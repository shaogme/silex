use silex_i18n::{I18nBuilder, Runtime, t};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
        let store = I18nBuilder::new(
            scope,
            scope.error_handler(|_| {}).expect("error handler"),
        )
            .build()
            .expect("valid store");
        let translation = t!(store, "missing.key").expect("translation");
            assert_eq!(translation.get().expect("translation value"), "missing.key");
        })
        .expect("child scope");
}
