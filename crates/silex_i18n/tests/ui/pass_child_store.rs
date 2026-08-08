use silex_i18n::{I18nBuilder, Runtime, t};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let store = I18nBuilder::new(scope, scope.error_handler(|_| {}))
            .build()
            .expect("valid store");
        let translation = t!(store, "missing.key");
        assert_eq!(translation.get(), "missing.key");
    });
}
