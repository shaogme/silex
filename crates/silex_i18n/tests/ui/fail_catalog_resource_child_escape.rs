use silex_i18n::{Catalog, CatalogResource, I18nBuilder, I18nError, Runtime};

fn escaped() -> CatalogResource<'static, I18nError> {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler = scope.error_handler(|_| {}).expect("error handler");
        let store = I18nBuilder::new(
            scope,
            handler.view(),
        )
            .build()
            .expect("valid store");
        store.catalog_resource(
            move |_locale| async {
                Err::<Catalog, I18nError>(I18nError::recoverable(
                    silex_i18n::I18nErrorKind::Loader("not loaded".to_string()),
                ))
            },
            None,
        )
        .expect("resource")
    })
    .expect("child scope")
}

fn main() {
    let _ = escaped();
}
