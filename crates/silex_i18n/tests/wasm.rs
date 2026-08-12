#![cfg(target_arch = "wasm32")]

use gloo_timers::future::TimeoutFuture;
use silex_core::{ErrorReporter, Runtime, Scope, SilexResult, runtime_inputs_of};
use silex_i18n::{
    Catalog, CatalogLoadError, I18nBuilder, I18nStore, Locale, ResourceState, SuspenseContext, t,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_test::*;

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope.error_handler(|_| {}).expect("error handler")
}

fn store<'scope>(scope: Scope<'scope>, locale: &str) -> I18nStore<'scope> {
    I18nBuilder::new(scope, test_handler(scope))
        .locale(Locale::new(locale).expect("valid locale"))
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
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, "en-US");
        let calls_for_loader = calls.clone();
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let resource = i18n
            .catalog_resource(
                move |locale| {
                    calls_for_loader.set(calls_for_loader.get() + 1);
                    async move {
                        wait_for_reactivity(0).await;
                        catalog(locale, "Loaded")
                    }
                },
                suspense,
            )
            .expect("catalog resource");

        assert!(suspense.count.get_untracked().expect("reactive value") > 0);
        wait_for_reactivity(10).await;
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert_eq!(calls.get(), 1);
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Ready(_)
        ));
        assert!(
            i18n.has_catalog(&Locale::new("en-US").expect("valid locale"))
                .expect("catalog lookup")
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test]
fn catalog_resource_rejects_foreign_suspense_before_allocating_nodes() {
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.run().expect("root scope");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run().expect("root scope");
    let foreign_scope = foreign_root.scope();
    let target_scope = target_root.scope();
    let i18n = store(target_scope, "en-US");
    let suspense = SuspenseContext::new(foreign_scope).expect("suspense context");
    let before = target_scope.runtime_snapshot();
    let result = i18n.catalog_resource(
        |_| async { Err::<Catalog, _>("foreign suspense must fail".to_string()) },
        suspense,
    );

    assert!(matches!(
        result,
        Err(silex_i18n::I18nError::Reactivity(
            silex_core::ReactiveError::RuntimeMismatch
        ))
    ));
    assert_eq!(target_scope.runtime_snapshot(), before);
    assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
    target_root.dispose().expect("target root cleanup");
    foreign_root.dispose().expect("foreign root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_uses_store_catalog_without_calling_loader() {
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = I18nBuilder::new(scope, test_handler(scope))
            .locale(Locale::new("en-US").expect("valid locale"))
            .catalog(
                catalog(Locale::new("en-US").expect("valid locale"), "Cached")
                    .expect("valid catalog"),
            )
            .build()
            .expect("valid i18n store");
        let calls_for_loader = calls.clone();
        let resource = i18n
            .catalog_resource(
                move |_| {
                    calls_for_loader.set(calls_for_loader.get() + 1);
                    async { Err::<Catalog, _>("loader must not run".to_string()) }
                },
                None,
            )
            .expect("catalog resource");

        wait_for_reactivity(0).await;
        assert_eq!(calls.get(), 0);
        assert!(
            resource
                .value()
                .expect("resource value")
                .expect("cached catalog")
                .get("title")
                .is_some()
        );
        assert!(
            i18n.has_catalog(&Locale::new("en-US").expect("valid locale"))
                .expect("catalog lookup")
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_refetch_uses_cache_without_incrementing_loader_calls() {
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, "en-US");
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let calls_for_loader = calls.clone();
        let resource = i18n
            .catalog_resource(
                move |locale| {
                    calls_for_loader.set(calls_for_loader.get() + 1);
                    async move { catalog(locale, "Cached after refetch") }
                },
                suspense,
            )
            .expect("catalog resource");

        wait_for_reactivity(0).await;
        assert_eq!(calls.get(), 1);
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Ready(_)
        ));

        resource.refetch().expect("resource refetch");
        assert!(resource.loading().expect("resource loading"));
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 1);
        wait_for_reactivity(0).await;

        assert_eq!(calls.get(), 1);
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert_eq!(
            resource
                .value()
                .expect("resource value")
                .expect("refetched catalog")
                .locale(),
            &Locale::new("en-US").expect("valid locale")
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_reports_loader_errors() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, "en-US");
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let resource = i18n.catalog_resource(
            |_| async { Err::<Catalog, _>("invalid catalog payload".to_string()) },
            suspense,
        )
        .expect("catalog resource");

        wait_for_reactivity(0).await;
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Error(CatalogLoadError::Loader(message)) if message == "invalid catalog payload"
        ));
        assert!(
            !i18n
                .has_catalog(&Locale::new("en-US").expect("valid locale"))
                .expect("catalog lookup")
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_error_refetch_balances_suspense_and_recovers() {
    let calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, "en-US");
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let calls_for_loader = calls.clone();
        let resource = i18n
            .catalog_resource(
                move |locale| {
                    let call = calls_for_loader.get() + 1;
                    calls_for_loader.set(call);
                    async move {
                        if call == 1 {
                            Err("temporary loader error".to_string())
                        } else {
                            catalog(locale, "Recovered")
                        }
                    }
                },
                suspense,
            )
            .expect("catalog resource");

        wait_for_reactivity(0).await;
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Error(CatalogLoadError::Loader(message))
                if message == "temporary loader error"
        ));
        assert!(
            !i18n
                .has_catalog(&Locale::new("en-US").expect("valid locale"))
                .expect("catalog lookup")
        );

        resource.refetch().expect("resource refetch");
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 1);
        wait_for_reactivity(0).await;

        assert_eq!(calls.get(), 2);
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Ready(ref catalog) if catalog.get("title").is_some()
        ));
        assert!(
            i18n.has_catalog(&Locale::new("en-US").expect("valid locale"))
                .expect("catalog lookup")
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_rejects_a_catalog_for_the_wrong_locale() {
    let requested = Locale::new("en-US").expect("valid locale");
    let loaded = Locale::new("zh-CN").expect("valid locale");
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, requested.as_str());
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let loaded_for_loader = loaded.clone();
        let resource = i18n
            .catalog_resource(
                move |_| {
                    let loaded = loaded_for_loader.clone();
                    async move { catalog(loaded, "Wrong locale") }
                },
                suspense,
            )
            .expect("catalog resource");

        wait_for_reactivity(0).await;
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Error(CatalogLoadError::LocaleMismatch {
                requested: ref actual_requested,
                loaded: ref actual_loaded,
            }) if actual_requested == &requested && actual_loaded == &loaded
        ));
        assert!(!i18n.has_catalog(&requested).expect("catalog lookup"));
        assert!(!i18n.has_catalog(&loaded).expect("catalog lookup"));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_discards_old_locale_response() {
    let en = Locale::new("en-US").expect("valid locale");
    let zh = Locale::new("zh-CN").expect("valid locale");
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, "en-US");
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let en_for_loader = en.clone();
        let resource = i18n
            .catalog_resource(
                move |locale| {
                    let slow = locale == en_for_loader;
                    async move {
                        wait_for_reactivity(if slow { 30 } else { 0 }).await;
                        catalog(locale, "Current")
                    }
                },
                suspense,
            )
            .expect("catalog resource");

        wait_for_reactivity(0).await;
        i18n.set_locale(zh.clone()).expect("locale update");
        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Reloading(_) | ResourceState::Loading
        ));
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 1);
        wait_for_reactivity(50).await;

        assert!(matches!(
            resource.state().get_untracked().expect("reactive value"),
            ResourceState::Ready(ref catalog) if catalog.locale() == &zh
        ));
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 0);
        assert!(i18n.has_catalog(&zh).expect("catalog lookup"));
        assert!(!i18n.has_catalog(&en).expect("catalog lookup"));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn catalog_resource_completion_is_cancelled_after_root_dispose() {
    let calls = Rc::new(Cell::new(0));
    let calls_before_dispose = calls.clone();
    let calls_for_loader = calls.clone();
    let resource_state_runs = Rc::new(Cell::new(0));
    let translation_runs = Rc::new(Cell::new(0));
    let resource_state_runs_for_scope = resource_state_runs.clone();
    let translation_runs_for_scope = translation_runs.clone();
    let resource_state_runs_at_dispose = Rc::new(Cell::new(0));
    let translation_runs_at_dispose = Rc::new(Cell::new(0));
    let resource_state_runs_at_dispose_for_scope = resource_state_runs_at_dispose.clone();
    let translation_runs_at_dispose_for_scope = translation_runs_at_dispose.clone();
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root scope");
    root.with_scope(|scope| async move {
        let i18n = store(scope, "en-US");
        let suspense = SuspenseContext::new(scope).expect("suspense context");
        let resource = i18n
            .catalog_resource(
                move |locale| {
                    calls_for_loader.set(calls_for_loader.get() + 1);
                    async move {
                        wait_for_reactivity(20).await;
                        catalog(locale, "Late")
                    }
                },
                suspense,
            )
            .expect("catalog resource");
        let resource_state = resource.state();
        let resource_state_runs_for_effect = resource_state_runs_for_scope.clone();
        scope
            .effect_from(
                runtime_inputs_of(resource_state),
                move || -> SilexResult<()> {
                    let _ = resource_state.get_untracked()?;
                    resource_state_runs_for_effect.set(resource_state_runs_for_effect.get() + 1);
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("resource state effect can be registered");
        let translation = t!(i18n, "title").expect("translation");
        let translation_runs_for_effect = translation_runs_for_scope.clone();
        scope
            .effect_from(
                runtime_inputs_of(translation),
                move || -> SilexResult<()> {
                    let _ = translation.get_untracked()?;
                    translation_runs_for_effect.set(translation_runs_for_effect.get() + 1);
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("translation effect can be registered");

        wait_for_reactivity(0).await;
        assert_eq!(calls_before_dispose.get(), 1);
        assert_eq!(suspense.count.get_untracked().expect("reactive value"), 1);
        assert_eq!(
            translation.get_untracked().expect("reactive value"),
            "title"
        );
        assert!(resource_state_runs_for_scope.get() > 0);
        assert!(translation_runs_for_scope.get() > 0);
        resource_state_runs_at_dispose_for_scope.set(resource_state_runs_for_scope.get());
        translation_runs_at_dispose_for_scope.set(translation_runs_for_scope.get());
    })
    .await;

    root.dispose().expect("root cleanup");
    wait_for_reactivity(30).await;
    assert_eq!(calls.get(), 1);
    assert_eq!(
        resource_state_runs.get(),
        resource_state_runs_at_dispose.get()
    );
    assert_eq!(translation_runs.get(), translation_runs_at_dispose.get());
}
