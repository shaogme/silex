use silex_i18n::{I18nBuilder, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler = scope.error_handler(|_| {}).expect("error handler");
        let _store = I18nBuilder::new(scope, handler);
    });
}
