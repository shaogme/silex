use silex_i18n::{I18nBuilder, Runtime, t};

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| {
        let handler = scope.error_handler(|_| {}).expect("error handler");
        let store = I18nBuilder::new(
            scope,
            handler.view(),
        )
            .build()
            .expect("valid store");
        let translation = t!(store, "missing.key").expect("translation");
        assert_eq!(translation.get().expect("translation value"), "missing.key");
    });
    root.dispose().expect("root cleanup");
}
