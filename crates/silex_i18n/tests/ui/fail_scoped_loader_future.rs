use silex_i18n::{Catalog, I18nBuilder, I18nError, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, _) = scope.signal(1_u32).expect("source signal");
        let handler = scope.error_handler(|_| {}).expect("error handler");
        let store = I18nBuilder::new(
            scope,
            handler.view(),
        )
            .build()
            .expect("valid store");
        let _resource = store.catalog_resource(
            move |_locale| async move {
                let _ = source.get();
                Err::<Catalog, I18nError>(I18nError::recoverable(
                    silex_i18n::I18nErrorKind::Loader("not loaded".to_string()),
                ))
            },
            None,
        );
    });
}
