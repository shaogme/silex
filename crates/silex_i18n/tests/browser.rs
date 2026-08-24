#![cfg(all(target_arch = "wasm32", feature = "browser-tests"))]

use gloo_timers::future::TimeoutFuture;
use silex_core::{ErrorHandlerInput, ErrorHandlerToken, OwnerAccess, Runtime, RxGet, SilexContext};
use silex_dom::browser::BrowserDom;
use silex_i18n::{Catalog, I18nBuilder, Locale, detect_browser_locale, t};
#[cfg(feature = "intl")]
use silex_i18n::{DateTimeFormat, format_number};
#[cfg(feature = "persist")]
use silex_router::{RouterContext, RouterContextProps};
use silex_view::{MountContext, MountOwnerToken};
use wasm_bindgen_test::*;
#[cfg(feature = "persist")]
use web_sys::StorageEvent;
use web_sys::window;

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner.error_handler(|_| {}).expect("error handler")
}

fn test_owner<'owner>(
    owner: OwnerAccess<'owner>,
) -> (MountOwnerToken<'owner>, ErrorHandlerToken<'owner>) {
    let error_handler = test_handler(owner);
    (MountOwnerToken::new(owner), error_handler)
}

#[cfg(feature = "persist")]
const BINDING_KEY: &str = "silex-i18n-wasm-binding";
#[cfg(feature = "persist")]
const EVENT_KEY: &str = "silex-i18n-wasm-storage-event";

fn store<'owner>(
    owner: silex_core::OwnerAccess<'owner>,
    locale: &str,
) -> (silex_i18n::I18nStore<'owner>, ErrorHandlerToken<'owner>) {
    let handler = test_handler(owner);
    let store = I18nBuilder::new(owner, handler.view())
        .locale(Locale::new(locale).expect("valid locale"))
        .build()
        .expect("valid i18n store");
    (store, handler)
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
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let storage = window()
            .expect("window")
            .local_storage()
            .expect("localStorage access")
            .expect("localStorage available");
        storage.remove_item(BINDING_KEY).expect("clear test key");

        let binding = silex_i18n::Persistent::builder(owner, BINDING_KEY, test_handler(owner))
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US").expect("valid locale"))
            .build()
            .expect("locale binding");
        binding
            .set(Locale::new("zh-CN").expect("valid locale"))
            .expect("binding update");
        binding.flush().expect("persist initial locale");

        let handler = test_handler(owner);
        let i18n = I18nBuilder::new(owner, handler.view())
            .locale(Locale::new("en-US").expect("valid locale"))
            .locale_binding(binding)
            .build()
            .expect("valid i18n store");
        assert_eq!(
            i18n.locale().get_untracked().expect("reactive value"),
            Locale::new("zh-CN").expect("valid locale")
        );

        i18n.set_locale(Locale::new("ar-EG").expect("valid locale"))
            .expect("locale update");
        assert_eq!(
            binding.get_untracked().expect("reactive value"),
            Locale::new("ar-EG").expect("valid locale")
        );
        assert_eq!(
            storage
                .get_item(BINDING_KEY)
                .expect("read persisted locale"),
            Some("ar-EG".to_string())
        );

        storage.remove_item(BINDING_KEY).expect("cleanup test key");
    });
    root.close().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn storage_event_updates_all_locale_bindings() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let window = window().expect("window");
        let storage = window
            .local_storage()
            .expect("localStorage access")
            .expect("localStorage available");
        storage.remove_item(EVENT_KEY).expect("clear test key");

        let first = silex_i18n::Persistent::builder(owner, EVENT_KEY, test_handler(owner))
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US").expect("valid locale"))
            .build()
            .expect("first locale binding");
        let second = silex_i18n::Persistent::builder(owner, EVENT_KEY, test_handler(owner))
            .local()
            .parse::<Locale>()
            .default(Locale::new("en-US").expect("valid locale"))
            .build()
            .expect("second locale binding");

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

        assert_eq!(first.get_untracked().expect("reactive value"), Locale::new("zh-CN").expect("valid locale"));
        assert_eq!(second.get_untracked().expect("reactive value"), Locale::new("zh-CN").expect("valid locale"));
        storage.remove_item(EVENT_KEY).expect("cleanup test key");
    });
    root.close().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test]
fn query_binding_follows_router_search_signal() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let path = owner.signal("/settings".to_string()).expect("path signals");
        let search = owner
            .signal("?lang=en-US".to_string())
            .expect("search signals");
        let context_handler = test_handler(owner);
        let ctx = RouterContext::new(
            SilexContext::new(owner, context_handler.view()),
            RouterContextProps {
                base_path: "/".to_string(),
                path: path.read_signal(),
                search: search.read_signal(),
                set_path: path.write_signal(),
                set_search: search.write_signal(),
            },
        )
        .expect("valid router ctx");
        let binding = silex_i18n::Persistent::builder(owner, "lang", test_handler(owner))
            .query(ctx)
            .parse::<Locale>()
            .default(Locale::new("en-US").expect("valid locale"))
            .build()
            .expect("locale binding");

        assert_eq!(
            binding.get_untracked().expect("reactive value"),
            Locale::new("en-US").expect("valid locale")
        );
        search
            .set("?lang=zh-CN".to_string())
            .expect("search update");
        assert_eq!(
            binding.get_untracked().expect("reactive value"),
            Locale::new("zh-CN").expect("valid locale")
        );
    });
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test]
fn browser_locale_and_document_metadata_use_real_window() {
    let available = [
        Locale::new("en-US").expect("valid locale"),
        Locale::new("zh-CN").expect("valid locale"),
    ];
    let fallback = Locale::new("en-US").expect("valid locale");
    let resolved = detect_browser_locale(&available, &fallback);
    assert!(available.contains(&resolved) || resolved == fallback);

    let mut runtime = Runtime::new();
    let root_handle = runtime.owner().expect("root owner");
    let (old_lang, old_dir) = root_handle.with_access(|owner| {
        let (i18n, _handler) = store(owner, "en-US");
        let root = window()
            .expect("window")
            .document()
            .expect("document")
            .document_element()
            .expect("document root");
        let old_lang = root.get_attribute("lang");
        let old_dir = root.get_attribute("dir");
        let _metadata = i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
        assert_eq!(root.get_attribute("lang"), Some("en-US".to_string()));
        assert_eq!(root.get_attribute("dir"), Some("ltr".to_string()));
        i18n.set_locale(Locale::new("ar-EG").expect("valid locale"))
            .expect("locale update");
        assert_eq!(root.get_attribute("lang"), Some("ar-EG".to_string()));
        assert_eq!(root.get_attribute("dir"), Some("rtl".to_string()));
        (old_lang, old_dir)
    });
    root_handle.close().expect("root cleanup");
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
    let root = runtime.owner().expect("root owner");
    let owner = root.access();
    async move {
        let en = Catalog::from_entries(
            Locale::new("en-US").expect("valid locale"),
            [("title", "English")],
        )
        .expect("valid English catalog");
        let zh = Catalog::from_entries(
            Locale::new("zh-CN").expect("valid locale"),
            [("title", "中文")],
        )
        .expect("valid Chinese catalog");
        let handler = test_handler(owner);
        let i18n = I18nBuilder::new(owner, handler.view())
            .locale(Locale::new("en-US").expect("valid locale"))
            .catalogs([en, zh])
            .build()
            .expect("valid i18n store");
        let parent = window()
            .expect("window")
            .document()
            .expect("document")
            .create_element("div")
            .expect("parent element");
        let (owner, error_handler) = test_owner(owner);
        let browser = BrowserDom::from_window().expect("browser backend");
        let parent_node = browser
            .from_web_sys_node(parent.clone().into())
            .expect("parent should be a DOM node");
        let context = MountContext::for_parent(
            browser.context(),
            parent_node,
            owner,
            error_handler.handler_ref(),
        );
        let translation = t!(i18n, "title").expect("translation");
        let _mount = context
            .mount(&translation)
            .expect("translation should mount");
        context
            .transaction()
            .commit()
            .expect("translation should commit");

        assert_eq!(parent.text_content(), Some("English".to_string()));
        assert_eq!(parent.child_nodes().length(), 1);
        i18n.set_locale(Locale::new("zh-CN").expect("valid locale"))
            .expect("locale update");
        wait_for_reactivity(0).await;
        assert_eq!(parent.text_content(), Some("中文".to_string()));
        assert_eq!(parent.child_nodes().length(), 1);
    }
    .await;
    root.close().expect("root cleanup");
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
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let catalog = Catalog::from_entries(
            Locale::new("en-US").expect("valid locale"),
            [("title", "English")],
        )
        .expect("valid catalog");
        let handler = test_handler(owner);
        let i18n = I18nBuilder::new(owner, handler.view())
            .locale(Locale::new("en-US").expect("valid locale"))
            .catalog(catalog)
            .build()
            .expect("valid i18n store");
        let (owner, error_handler) = test_owner(owner);
        let browser = BrowserDom::from_window().expect("browser backend");
        let parent_node = browser
            .from_web_sys_node(parent.clone().into())
            .expect("parent should be a DOM node");
        let context = MountContext::for_parent(
            browser.context(),
            parent_node,
            owner,
            error_handler.handler_ref(),
        );
        let translation = t!(i18n, "title").expect("translation");
        let _mount = context
            .mount(&translation)
            .expect("translation should mount");
        context
            .transaction()
            .commit()
            .expect("translation should commit");
        assert_eq!(parent.text_content(), Some("English".to_string()));
        assert_eq!(parent.child_nodes().length(), 1);
    });

    root.close().expect("root cleanup");
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
    let target_root = target_runtime.owner().expect("root owner");
    let mut foreign_runtime = Runtime::new();
    let foreign_root = foreign_runtime.owner().expect("root owner");
    let target_scope = target_root.access();
    let catalog = Catalog::from_entries(
        Locale::new("en-US").expect("valid locale"),
        [("title", "English")],
    )
    .expect("valid catalog");
    let handler = test_handler(target_scope);
    let i18n = I18nBuilder::new(target_scope, handler.view())
        .locale(Locale::new("en-US").expect("valid locale"))
        .catalog(catalog)
        .build()
        .expect("valid i18n store");
    {
        let foreign_scope = foreign_root.access();
        let translation = t!(i18n, "title").expect("translation");
        let (owner, error_handler) = test_owner(foreign_scope);
        let browser = BrowserDom::from_window().expect("browser backend");
        let parent_node = browser
            .from_web_sys_node(parent.clone().into())
            .expect("parent should be a DOM node");
        let context = MountContext::for_parent(
            browser.context(),
            parent_node,
            owner,
            error_handler.handler_ref(),
        );
        assert!(context.mount(&translation).is_err());
    }

    assert!(parent.first_child().is_none());
    drop(handler);
    target_root.close().expect("target root cleanup");
    foreign_root.close().expect("foreign root cleanup");
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
    let first_root = first_runtime.owner().expect("root owner");
    first_root.with_access(|owner| {
        let (i18n, _handler) = store(owner, "en-US");
        let _metadata = i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
    });

    let mut second_runtime = Runtime::new();
    let second_root = second_runtime.owner().expect("root owner");
    second_root.with_access(|owner| {
        let (i18n, _handler) = store(owner, "zh-CN");
        let _metadata = i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
    });

    assert_eq!(
        document_root.get_attribute("lang"),
        Some("zh-CN".to_string())
    );
    assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    first_root.close().expect("first root cleanup");
    assert_eq!(
        document_root.get_attribute("lang"),
        Some("zh-CN".to_string())
    );
    assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    second_root.close().expect("second root cleanup");
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
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let (i18n, _handler) = store(owner, "en-US");
        let _metadata = i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
        document_root
            .set_attribute("lang", "external")
            .expect("set external lang");
        document_root
            .set_attribute("dir", "external")
            .expect("set external dir");
    });

    root.close().expect("root cleanup");
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
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let (i18n, _handler) = store(owner, "en-US");
        let metadata = i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
        metadata.stop().expect("stop metadata effect");
        i18n.set_locale(Locale::new("ar-EG").expect("valid locale"))
            .expect("locale update");
        assert_eq!(
            document_root.get_attribute("lang"),
            Some("en-US".to_string())
        );
        assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    });

    root.close().expect("root cleanup");
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
    let first_root = first_runtime.owner().expect("root owner");
    let mut second_runtime = Runtime::new();
    let second_root = second_runtime.owner().expect("root owner");

    first_root.with_access(|first_scope| {
        let (first_i18n, _first_handler) = store(first_scope, "en-US");
        let _first_metadata = first_i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
        second_root.with_access(|second_scope| {
            let (second_i18n, _second_handler) = store(second_scope, "zh-CN");
            let _second_metadata = second_i18n
                .sync_document_metadata()
                .expect("metadata effect can be registered");

            first_i18n
                .set_locale(Locale::new("fr-FR").expect("valid locale"))
                .expect("locale update");
            assert_eq!(
                document_root.get_attribute("lang"),
                Some("zh-CN".to_string())
            );
            assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
        });

        second_root.close().expect("second root cleanup");
        assert_eq!(
            document_root.get_attribute("lang"),
            Some("fr-FR".to_string())
        );
        assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    });

    first_root.close().expect("first root cleanup");
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
    let root = runtime.owner().expect("root owner");
    root.with_access(|owner| {
        let (i18n, _handler) = store(owner, "en-US");
        let metadata = i18n
            .sync_document_metadata()
            .expect("metadata effect can be registered");
        metadata.stop().expect("stop metadata effect");
        metadata.stop().expect("stop metadata effect");
        assert_eq!(
            document_root.get_attribute("lang"),
            Some("en-US".to_string())
        );
        assert_eq!(document_root.get_attribute("dir"), Some("ltr".to_string()));
    });

    root.close().expect("root cleanup");
    assert_eq!(document_root.get_attribute("lang"), old_lang);
    assert_eq!(document_root.get_attribute("dir"), old_dir);
}

#[cfg(feature = "intl")]
#[wasm_bindgen_test]
fn intl_formatters_use_the_browser_implementation() {
    let number = format_number(&Locale::new("en-US").expect("valid locale"), 1_234.5)
        .expect("format number");
    assert!(number.contains('1'));
    assert!(number.contains('4'));

    let date = DateTimeFormat::new(Locale::new("en-US").expect("valid locale"))
        .format(0.0)
        .expect("format date");
    assert!(!date.is_empty());
}
