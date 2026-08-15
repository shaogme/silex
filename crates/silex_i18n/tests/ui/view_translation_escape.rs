use silex_dom::view::AnyView;
use silex_i18n::{I18nBuilder, Runtime, t};

fn escaped() -> AnyView<'static> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler = scope.error_handler(|_| {}).expect("error handler");
        let store = I18nBuilder::new(
            scope,
            handler.view(),
        )
            .build()
            .expect("valid store");
        AnyView::new(t!(store, "missing.key").expect("translation"))
    })
    .expect("child scope should initialize")
}

fn main() {
    let _ = escaped();
}
