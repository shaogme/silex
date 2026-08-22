use silex_core::RxGet;
use silex_i18n::{I18nBuilder, Runtime, t};

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let handler = owner.error_handler(|_| {}).expect("error handler");
        let store = I18nBuilder::new(owner, handler.view())
            .build()
            .expect("valid store");
        let translation = t!(store, "missing.key").expect("translation");
        assert_eq!(translation.get().expect("translation value"), "missing.key");
    });
    root.close().expect("root cleanup");
}
