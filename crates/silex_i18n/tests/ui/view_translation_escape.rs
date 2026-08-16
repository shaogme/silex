use silex_dom::view::AnyView;
use silex_i18n::{I18nBuilder, Runtime, t};

fn escaped() -> AnyView<'static> {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let handler = owner.error_handler(|_| {}).expect("error handler");
            let store = I18nBuilder::new(owner, handler.view())
                .build()
                .expect("valid store");
            AnyView::new(t!(store, "missing.key").expect("translation"))
        })
        .expect("transient owner should initialize")
}

fn main() {
    let _ = escaped();
}
