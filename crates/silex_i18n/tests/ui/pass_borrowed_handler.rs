use silex_core::Runtime;
use silex_i18n::{I18nBuilder, Locale, t};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let token = scope.error_handler(|_| {}).expect("error handler");
            let view = token.view();

            let borrowed_store = I18nBuilder::new(scope, token.view())
                .locale(Locale::new("en-US").expect("valid locale"))
                .build()
                .expect("borrowed handler store");
            let _borrowed_translation = t!(borrowed_store, "title").expect("translation memo");

            let view_store = I18nBuilder::new(scope, view)
                .locale(Locale::new("en-US").expect("valid locale"))
                .build()
                .expect("view handler store");
            let _view_translation = t!(view_store, "title").expect("translation memo");
        })
        .expect("child scope");
}
