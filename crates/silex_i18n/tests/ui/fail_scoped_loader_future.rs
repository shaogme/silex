use silex_i18n::{Catalog, I18nBuilder, I18nError, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, _) = scope.signal(1_u32);
        let store = I18nBuilder::new(scope, silex_core::ErrorReporter::new(|_| {}))
            .build()
            .expect("valid store");
        let _resource = store.catalog_resource(
            move |_locale| async move {
                let _ = source.get();
                Err::<Catalog, I18nError>(I18nError::Loader("not loaded".to_string()))
            },
            None,
        );
    });
}
