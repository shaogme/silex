#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{
    reactivity::{create_detached_scope, dispose},
    traits::RxGet,
};
use silex_i18n::{
    Catalog, CatalogLoadError, I18nBuilder, I18nStore, Locale, ResourceState, SuspenseContext,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_test::*;

fn store(locale: &str) -> I18nStore {
    I18nBuilder::new()
        .locale(Locale::new(locale))
        .build()
        .expect("valid i18n store")
}

fn catalog(locale: Locale, title: &str) -> Result<Catalog, String> {
    Catalog::from_entries(locale, [("title", title)]).map_err(|error| error.to_string())
}

async fn wait_for_reactivity(milliseconds: u32) {
    TimeoutFuture::new(milliseconds).await;
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_loads_and_updates_suspense() {
    let calls = Rc::new(Cell::new(0));
    let (scope, (i18n, resource, suspense)) = create_detached_scope(|| {
        let i18n = store("en-US");
        let calls_for_loader = calls.clone();
        let suspense = SuspenseContext::new();
        let resource = i18n.catalog_resource(
            move |locale| {
                calls_for_loader.set(calls_for_loader.get() + 1);
                async move {
                    wait_for_reactivity(0).await;
                    catalog(locale, "Loaded")
                }
            },
            suspense,
        );
        (i18n, resource, suspense)
    });

    assert!(suspense.count.get_untracked() > 0);
    wait_for_reactivity(10).await;
    assert_eq!(suspense.count.get_untracked(), 0);
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        resource.state().get_untracked(),
        ResourceState::Ready(_)
    ));
    assert!(i18n.has_catalog(&Locale::new("en-US")));
    dispose(scope);
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_uses_store_catalog_without_calling_loader() {
    let calls = Rc::new(Cell::new(0));
    let (scope, (i18n, resource)) = create_detached_scope(|| {
        let i18n = I18nBuilder::new()
            .locale(Locale::new("en-US"))
            .catalog(catalog(Locale::new("en-US"), "Cached").expect("valid catalog"))
            .build()
            .expect("valid i18n store");
        let calls_for_loader = calls.clone();
        let resource = i18n.catalog_resource(
            move |_| {
                calls_for_loader.set(calls_for_loader.get() + 1);
                async { Err::<Catalog, _>("loader must not run".to_string()) }
            },
            None,
        );
        (i18n, resource)
    });

    wait_for_reactivity(0).await;
    assert_eq!(calls.get(), 0);
    assert!(
        resource
            .value()
            .expect("cached catalog")
            .get("title")
            .is_some()
    );
    assert!(i18n.has_catalog(&Locale::new("en-US")));
    dispose(scope);
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_reports_loader_errors() {
    let (scope, (i18n, resource)) = create_detached_scope(|| {
        let i18n = store("en-US");
        let resource = i18n.catalog_resource(
            |_| async { Err::<Catalog, _>("invalid catalog payload".to_string()) },
            None,
        );
        (i18n, resource)
    });

    wait_for_reactivity(0).await;
    assert!(matches!(
        resource.state().get_untracked(),
        ResourceState::Error(CatalogLoadError::Loader(message)) if message == "invalid catalog payload"
    ));
    assert!(!i18n.has_catalog(&Locale::new("en-US")));
    dispose(scope);
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_discards_old_locale_response() {
    let en = Locale::new("en-US");
    let zh = Locale::new("zh-CN");
    let (scope, (i18n, resource)) = create_detached_scope(|| {
        let i18n = store("en-US");
        let en_for_loader = en.clone();
        let resource = i18n.catalog_resource(
            move |locale| {
                let slow = locale == en_for_loader;
                async move {
                    wait_for_reactivity(if slow { 30 } else { 0 }).await;
                    catalog(locale, "Current")
                }
            },
            None,
        );
        (i18n, resource)
    });

    wait_for_reactivity(0).await;
    i18n.set_locale(zh.clone());
    assert!(matches!(
        resource.state().get_untracked(),
        ResourceState::Reloading(_) | ResourceState::Loading
    ));
    wait_for_reactivity(50).await;

    assert!(matches!(
        resource.state().get_untracked(),
        ResourceState::Ready(ref catalog) if catalog.locale() == &zh
    ));
    assert!(i18n.has_catalog(&zh));
    assert!(!i18n.has_catalog(&en));
    dispose(scope);
}
