#![cfg(all(target_arch = "wasm32", feature = "browser-tests"))]

use gloo_timers::future::TimeoutFuture;
use silex_core::{Runtime, traits::RxGet};
use silex_dom::view::{ScopedViewOwner, View};
use silex_i18n::{Catalog, I18nBuilder, Locale, detect_browser_locale, t};
#[cfg(feature = "intl")]
use silex_i18n::{DateTimeFormat, format_number};
#[cfg(feature = "persist")]
use silex_router::{RouterContext, RouterContextProps};
use wasm_bindgen_test::*;
#[cfg(feature = "persist")]
use web_sys::StorageEvent;
use web_sys::window;

wasm_bindgen_test_configure!(run_in_browser);

#[cfg(feature = "persist")]
const BINDING_KEY: &str = "silex-i18n-wasm-binding";
#[cfg(feature = "persist")]
const EVENT_KEY: &str = "silex-i18n-wasm-storage-event";

fn store<'scope>(scope: silex_core::Scope<'scope>, locale: &str) -> silex_i18n::I18nStore<'scope> {
    I18nBuilder::new(scope)
        .locale(Locale::new(locale))
        .build()
        .expect("valid i18n store")
}

async fn wait_for_reactivity(milliseconds: u32) {
    TimeoutFuture::new(milliseconds).await;
}

fn restore_attribute(root: &web_sys::Element, name: &str, value: Option<&str>) {
    match value {
        Some(value) => root.set_attribute(name, value).expect("restore attribute"),
        None => root.remove_attribute(name).expect("restore attribute"),
    }
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn local_storage_binding_round_trips_locale() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let storage = window()
            .expect("window")
            .local_storage()
            .expect("localStorage access")
            .expect("localStorage available");
        storage.remove_item(BINDING_KEY).expect("clear test key");

        let binding = silex_i18n::Persistent::builder(scope, BINDING_KEY)
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();
        binding.set(Locale::new("zh-CN"));
        binding.flush().expect("persist initial locale");

        let i18n = I18nBuilder::new(scope)
            .locale(Locale::new("en-US"))
            .locale_binding(binding)
            .build()
            .expect("valid i18n store");
        assert_eq!(i18n.locale().get_untracked(), Locale::new("zh-CN"));

        i18n.set_locale(Locale::new("ar-EG"));
        assert_eq!(binding.get_untracked(), Locale::new("ar-EG"));
        assert_eq!(
            storage
                .get_item(BINDING_KEY)
                .expect("read persisted locale"),
            Some("ar-EG".to_string())
        );

        storage.remove_item(BINDING_KEY).expect("cleanup test key");
    });
    root.dispose().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn storage_event_updates_all_locale_bindings() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let window = window().expect("window");
        let storage = window
            .local_storage()
            .expect("localStorage access")
            .expect("localStorage available");
        storage.remove_item(EVENT_KEY).expect("clear test key");

        let first = silex_i18n::Persistent::builder(scope, EVENT_KEY)
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();
        let second = silex_i18n::Persistent::builder(scope, EVENT_KEY)
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();

        let old_value = storage
            .get_item(EVENT_KEY)
            .expect("read current storage value");
        storage
            .set_item(EVENT_KEY, "zh-CN")
            .expect("set external storage value");

        let event = StorageEvent::new("storage").expect("create storage event");
        event.init_storage_event_with_can_bubble_and_cancelable_and_key_and_old_value_and_new_value_and_url_and_storage_area(
            "storage",
            false,
            false,
            Some(EVENT_KEY),
            old_value.as_deref(),
            Some("zh-CN"),
            Some("https://example.test/"),
            Some(&storage),
        );
        window
            .dispatch_event(event.as_ref())
            .expect("dispatch storage event");

        assert_eq!(first.get_untracked(), Locale::new("zh-CN"));
        assert_eq!(second.get_untracked(), Locale::new("zh-CN"));
        storage.remove_item(EVENT_KEY).expect("cleanup test key");
    });
    root.dispose().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn query_binding_follows_router_search_signal() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let (path, set_path) = scope.signal("/settings".to_string());
        let (search, set_search) = scope.signal("?lang=en-US".to_string());
        let ctx = RouterContext::try_new(
            scope,
            RouterContextProps {
                base_path: "/".to_string(),
                path,
                search,
                set_path,
                set_search,
            },
        )
        .expect("valid router context");
        let binding = silex_i18n::Persistent::builder(scope, "lang")
            .query(&ctx)
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();

        assert_eq!(binding.get_untracked(), Locale::new("en-US"));
        set_search.set("?lang=zh-CN".to_string());
        assert_eq!(binding.get_untracked(), Locale::new("zh-CN"));
    });
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test]
fn browser_locale_and_document_metadata_use_real_window() {
    let available = [Locale::new("en-US"), Locale::new("zh-CN")];
    let fallback = Locale::new("en-US");
    let resolved = detect_browser_locale(&available, &fallback);
    assert!(available.contains(&resolved) || resolved == fallback);

    let mut runtime = Runtime::new();
    let root_handle = runtime.run();
    let (old_lang, old_dir) = root_handle.with_scope(|scope| {
        let i18n = store(scope, "en-US");
        let root = window()
            .expect("window")
            .document()
            .expect("document")
            .document_element()
            .expect("document root");
        let old_lang = root.get_attribute("lang");
        let old_dir = root.get_attribute("dir");
        let _metadata = i18n.sync_document_metadata();
        assert_eq!(root.get_attribute("lang"), Some("en-US".to_string()));
        assert_eq!(root.get_attribute("dir"), Some("ltr".to_string()));
        i18n.set_locale(Locale::new("ar-EG"));
        assert_eq!(root.get_attribute("lang"), Some("ar-EG".to_string()));
        assert_eq!(root.get_attribute("dir"), Some("rtl".to_string()));
        (old_lang, old_dir)
    });
    root_handle.dispose().expect("root cleanup");
    let document_root = window()
        .expect("window")
        .document()
        .expect("document")
        .document_element()
        .expect("document root");
    assert_eq!(document_root.get_attribute("lang"), old_lang);
    assert_eq!(document_root.get_attribute("dir"), old_dir);
}

#[wasm_bindgen_test(async)]
async fn translated_memo_updates_the_existing_text_node() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let en = Catalog::from_entries(Locale::new("en-US"), [("title", "English")])
            .expect("valid English catalog");
        let zh = Catalog::from_entries(Locale::new("zh-CN"), [("title", "中文")])
            .expect("valid Chinese catalog");
        let i18n = I18nBuilder::new(scope)
            .locale(Locale::new("en-US"))
            .catalogs([en, zh])
            .build()
            .expect("valid i18n store");
        let parent = window()
            .expect("window")
            .document()
            .expect("document")
            .create_element("div")
            .expect("parent element");
        let owner = ScopedViewOwner::new(scope);
        t!(i18n, "title").mount_owned(&owner, parent.as_ref(), Vec::new());

        assert_eq!(parent.text_content(), Some("English".to_string()));
        assert_eq!(parent.child_nodes().length(), 1);
        i18n.set_locale(Locale::new("zh-CN"));
        wait_for_reactivity(0).await;
        assert_eq!(parent.text_content(), Some("中文".to_string()));
        assert_eq!(parent.child_nodes().length(), 1);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test]
fn translated_memo_is_removed_when_its_root_is_disposed() {
    let parent = window()
        .expect("window")
        .document()
        .expect("document")
        .create_element("div")
        .expect("parent element");
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let catalog = Catalog::from_entries(Locale::new("en-US"), [("title", "English")])
            .expect("valid catalog");
        let i18n = I18nBuilder::new(scope)
            .locale(Locale::new("en-US"))
            .catalog(catalog)
            .build()
            .expect("valid i18n store");
        let owner = ScopedViewOwner::new(scope);
        t!(i18n, "title").mount_owned(&owner, parent.as_ref(), Vec::new());
        assert_eq!(parent.text_content(), Some("English".to_string()));
        assert_eq!(parent.child_nodes().length(), 1);
    });

    root.dispose().expect("root cleanup");
    assert!(parent.first_child().is_none());
}

#[wasm_bindgen_test]
fn foreign_translation_source_does_not_mount_or_allocate_foreign_owner_nodes() {
    let parent = window()
        .expect("window")
        .document()
        .expect("document")
        .create_element("div")
        .expect("parent element");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run();
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.run();
    let target_scope = target_root.scope();
    let foreign_scope = foreign_root.scope();
    let catalog =
        Catalog::from_entries(Locale::new("en-US"), [("title", "English")]).expect("valid catalog");
    let i18n = I18nBuilder::new(target_scope)
        .locale(Locale::new("en-US"))
        .catalog(catalog)
        .build()
        .expect("valid i18n store");
    let translation = t!(i18n, "title");
    let owner = ScopedViewOwner::new(foreign_scope);

    translation.mount_owned(&owner, parent.as_ref(), Vec::new());

    assert!(parent.first_child().is_none());
    target_root.dispose().expect("target root cleanup");
    foreign_root.dispose().expect("foreign root cleanup");
}

#[wasm_bindgen_test]
fn metadata_owner_cleanup_does_not_overwrite_newer_owner() {
    let document_root = window()
        .expect("window")
        .document()
        .expect("document")
        .document_element()
        .expect("document root");
    let old_lang = document_root.get_attribute("lang");
    let old_dir = document_root.get_attribute("dir");

    let mut first_runtime = Runtime::new();
    let first_root = first_runtime.run();
    first_root.with_scope(|scope| {
        let i18n = store(scope, "en-US");
        let _metadata = i18n.sync_document_metadata();
    });

    let mut second_runtime = Runtime::new();
    let second_root = second_runtime.run();
    second_root.with_scope(|scope| {
        let i18n = store(scope, "zh-CN");
        let _metadata = i18n.sync_document_metadata();
    });

    assert_eq!(
        document_root.get_attribute("lang"),
        Some("zh-CN".to_string())
    );
    assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    first_root.dispose().expect("first root cleanup");
    assert_eq!(
        document_root.get_attribute("lang"),
        Some("zh-CN".to_string())
    );
    assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    second_root.dispose().expect("second root cleanup");
    assert_eq!(document_root.get_attribute("lang"), old_lang);
    assert_eq!(document_root.get_attribute("dir"), old_dir);
}

#[wasm_bindgen_test]
fn metadata_cleanup_preserves_external_attribute_changes() {
    let document_root = window()
        .expect("window")
        .document()
        .expect("document")
        .document_element()
        .expect("document root");
    let old_lang = document_root.get_attribute("lang");
    let old_dir = document_root.get_attribute("dir");
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let i18n = store(scope, "en-US");
        let _metadata = i18n.sync_document_metadata();
        document_root
            .set_attribute("lang", "external")
            .expect("set external lang");
        document_root
            .set_attribute("dir", "external")
            .expect("set external dir");
    });

    root.dispose().expect("root cleanup");
    assert_eq!(
        document_root.get_attribute("lang"),
        Some("external".to_string())
    );
    assert_eq!(
        document_root.get_attribute("dir"),
        Some("external".to_string())
    );
    restore_attribute(&document_root, "lang", old_lang.as_deref());
    restore_attribute(&document_root, "dir", old_dir.as_deref());
}

#[wasm_bindgen_test]
fn metadata_effect_stop_prevents_later_locale_updates() {
    let document_root = window()
        .expect("window")
        .document()
        .expect("document")
        .document_element()
        .expect("document root");
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let i18n = store(scope, "en-US");
        let metadata = i18n.sync_document_metadata();
        metadata.stop();
        i18n.set_locale(Locale::new("ar-EG"));
        assert_eq!(
            document_root.get_attribute("lang"),
            Some("en-US".to_string())
        );
        assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    });

    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test]
fn metadata_owner_reclaims_latest_locale_after_newer_owner_disposes() {
    let document_root = window()
        .expect("window")
        .document()
        .expect("document")
        .document_element()
        .expect("document root");
    let old_lang = document_root.get_attribute("lang");
    let old_dir = document_root.get_attribute("dir");

    let mut first_runtime = Runtime::new();
    let first_root = first_runtime.run();
    let mut second_runtime = Runtime::new();
    let second_root = second_runtime.run();

    first_root.with_scope(|first_scope| {
        let first_i18n = store(first_scope, "en-US");
        let _first_metadata = first_i18n.sync_document_metadata();
        second_root.with_scope(|second_scope| {
            let second_i18n = store(second_scope, "zh-CN");
            let _second_metadata = second_i18n.sync_document_metadata();

            first_i18n.set_locale(Locale::new("fr-FR"));
            assert_eq!(
                document_root.get_attribute("lang"),
                Some("zh-CN".to_string())
            );
            assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
        });

        second_root.dispose().expect("second root cleanup");
        assert_eq!(
            document_root.get_attribute("lang"),
            Some("fr-FR".to_string())
        );
        assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    });

    first_root.dispose().expect("first root cleanup");
    assert_eq!(document_root.get_attribute("lang"), old_lang);
    assert_eq!(document_root.get_attribute("dir"), old_dir);
}

#[wasm_bindgen_test]
fn metadata_stop_and_scope_cleanup_are_idempotent() {
    let document_root = window()
        .expect("window")
        .document()
        .expect("document")
        .document_element()
        .expect("document root");
    let old_lang = document_root.get_attribute("lang");
    let old_dir = document_root.get_attribute("dir");
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        let i18n = store(scope, "en-US");
        let metadata = i18n.sync_document_metadata();
        metadata.stop();
        metadata.stop();
        assert_eq!(
            document_root.get_attribute("lang"),
            Some("en-US".to_string())
        );
        assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    });

    root.dispose().expect("root cleanup");
    assert_eq!(document_root.get_attribute("lang"), old_lang);
    assert_eq!(document_root.get_attribute("dir"), old_dir);
}

#[cfg(feature = "intl")]
#[wasm_bindgen_test]
fn intl_formatters_use_the_browser_implementation() {
    let number = format_number(&Locale::new("en-US"), 1_234.5).expect("format number");
    assert!(number.contains('1'));
    assert!(number.contains('4'));

    let date = DateTimeFormat::new(Locale::new("en-US"))
        .format(0.0)
        .expect("format date");
    assert!(!date.is_empty());
}
