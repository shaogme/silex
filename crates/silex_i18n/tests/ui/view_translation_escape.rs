use silex_dom::view::AnyView;
use silex_i18n::{I18nBuilder, Runtime, t};

fn escaped() -> AnyView<'static> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let store = I18nBuilder::new(scope, scope.error_handler(|_| {}))
            .build()
            .expect("valid store");
        AnyView::new(t!(store, "missing.key"))
    })
}

fn main() {
    let _ = escaped();
}
