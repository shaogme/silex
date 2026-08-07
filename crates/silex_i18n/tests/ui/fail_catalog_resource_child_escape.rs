use silex_i18n::{Catalog, CatalogResource, I18nBuilder, I18nError, Runtime};

fn escaped() -> CatalogResource<'static, I18nError> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let store = I18nBuilder::new(scope, silex_core::ErrorReporter::new(|_| {}))
            .build()
            .expect("valid store");
        store.catalog_resource(
            |_locale| async {
                Err::<Catalog, I18nError>(I18nError::Loader("not loaded".to_string()))
            },
            None,
        )
    })
}

fn main() {
    let _ = escaped();
}
