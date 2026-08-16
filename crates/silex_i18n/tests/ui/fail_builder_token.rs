use silex_i18n::{I18nBuilder, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let handler = owner.error_handler(|_| {}).expect("error handler");
        let _store = I18nBuilder::new(owner, handler);
    });
}
