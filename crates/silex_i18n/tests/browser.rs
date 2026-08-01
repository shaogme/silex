#![cfg(all(target_arch = "wasm32", feature = "browser-tests"))]

use gloo_timers::future::TimeoutFuture;
#[cfg(feature = "persist")]
use silex_core::{reactivity::Signal, traits::RxWrite};
use silex_core::{
    reactivity::{create_detached_scope, dispose},
    traits::RxGet,
};
use silex_dom::view::View;
use silex_i18n::{Catalog, I18nBuilder, I18nStore, Locale, detect_browser_locale, t};
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

fn store(locale: &str) -> I18nStore {
    I18nBuilder::new()
        .locale(Locale::new(locale))
        .build()
        .expect("valid i18n store")
}

async fn wait_for_reactivity(milliseconds: u32) {
    TimeoutFuture::new(milliseconds).await;
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn local_storage_binding_round_trips_locale() {
    let (scope, values) = create_detached_scope(|| {
        let storage = window()
            .expect("window")
            .local_storage()
            .expect("localStorage access")
            .expect("localStorage available");
        storage.remove_item(BINDING_KEY).expect("clear test key");

        let binding = silex_i18n::Persistent::builder(BINDING_KEY)
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();
        binding.set(Locale::new("zh-CN"));
        binding.flush().expect("persist initial locale");

        let i18n = I18nBuilder::new()
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

        (storage, binding)
    });

    values.0.remove_item(BINDING_KEY).expect("cleanup test key");
    dispose(scope);
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn storage_event_updates_all_locale_bindings() {
    let (scope, values) = create_detached_scope(|| {
        let window = window().expect("window");
        let storage = window
            .local_storage()
            .expect("localStorage access")
            .expect("localStorage available");
        storage.remove_item(EVENT_KEY).expect("clear test key");

        let first = silex_i18n::Persistent::builder(EVENT_KEY)
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();
        let second = silex_i18n::Persistent::builder(EVENT_KEY)
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

        let values = (first.get_untracked(), second.get_untracked());
        storage.remove_item(EVENT_KEY).expect("cleanup test key");
        values
    });

    assert_eq!(values.0, Locale::new("zh-CN"));
    assert_eq!(values.1, Locale::new("zh-CN"));
    dispose(scope);
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn query_binding_follows_router_search_signal() {
    let (scope, value) = create_detached_scope(|| {
        let (path, set_path) = Signal::pair("/settings".to_string());
        let (search, set_search) = Signal::pair("?lang=en-US".to_string());
        let ctx = RouterContext::new(RouterContextProps {
            base_path: "/".to_string(),
            path,
            search,
            set_path,
            set_search,
        });
        let binding = silex_i18n::Persistent::builder("lang")
            .query(&ctx)
            .parse::<Locale>()
            .default(Locale::new("en-US"))
            .build();

        assert_eq!(binding.get_untracked(), Locale::new("en-US"));
        set_search.set("?lang=zh-CN".to_string());
        binding.get_untracked()
    });

    assert_eq!(value, Locale::new("zh-CN"));
    dispose(scope);
}

#[wasm_bindgen_test]
fn browser_locale_and_document_metadata_use_real_window() {
    let available = [Locale::new("en-US"), Locale::new("zh-CN")];
    let fallback = Locale::new("en-US");
    let resolved = detect_browser_locale(&available, &fallback);
    assert!(available.contains(&resolved) || resolved == fallback);

    let (scope, (i18n, root, old_lang, old_dir)) = create_detached_scope(|| {
        let i18n = store("en-US");
        let root = window()
            .expect("window")
            .document()
            .expect("document")
            .document_element()
            .expect("document root");
        let old_lang = root.get_attribute("lang");
        let old_dir = root.get_attribute("dir");
        i18n.sync_document_metadata();
        (i18n, root, old_lang, old_dir)
    });

    assert_eq!(root.get_attribute("lang"), Some("en-US".to_string()));
    assert_eq!(root.get_attribute("dir"), Some("ltr".to_string()));
    i18n.set_locale(Locale::new("ar-EG"));
    assert_eq!(root.get_attribute("lang"), Some("ar-EG".to_string()));
    assert_eq!(root.get_attribute("dir"), Some("rtl".to_string()));

    match old_lang {
        Some(value) => root.set_attribute("lang", &value).expect("restore lang"),
        None => root.remove_attribute("lang").expect("restore lang"),
    }
    match old_dir {
        Some(value) => root.set_attribute("dir", &value).expect("restore dir"),
        None => root.remove_attribute("dir").expect("restore dir"),
    }
    dispose(scope);
}

#[wasm_bindgen_test(async)]
async fn translated_memo_updates_the_existing_text_node() {
    let (scope, (i18n, parent)) = create_detached_scope(|| {
        let en = Catalog::from_entries(Locale::new("en-US"), [("title", "English")])
            .expect("valid English catalog");
        let zh = Catalog::from_entries(Locale::new("zh-CN"), [("title", "中文")])
            .expect("valid Chinese catalog");
        let i18n = I18nBuilder::new()
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
        t!(i18n, "title").mount(parent.as_ref(), Vec::new());
        (i18n, parent)
    });

    assert_eq!(parent.text_content(), Some("English".to_string()));
    assert_eq!(parent.child_nodes().length(), 1);
    i18n.set_locale(Locale::new("zh-CN"));
    wait_for_reactivity(0).await;
    assert_eq!(parent.text_content(), Some("中文".to_string()));
    assert_eq!(parent.child_nodes().length(), 1);
    dispose(scope);
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
