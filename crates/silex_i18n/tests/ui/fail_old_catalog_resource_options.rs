use silex_i18n::{Catalog, I18nBuilder, I18nError, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let handler = owner.error_handler(|_| {}).expect("error handler");
        let store = I18nBuilder::new(owner, handler.view())
            .build()
            .expect("valid store");
        let _resource = store.catalog_resource(
            |_| async {
                Ok::<Catalog, I18nError>(
                    Catalog::from_entries(
                        silex_i18n::Locale::new("en").expect("valid locale"),
                        [("title", "Title")],
                    )
                    .expect("valid catalog"),
                )
            },
            None,
        );
    });
}
