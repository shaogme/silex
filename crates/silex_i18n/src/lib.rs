#![doc = "Silex internationalization runtime."]

#[cfg(feature = "browser")]
mod browser;
mod catalog;
mod error;
#[cfg(feature = "intl")]
mod intl;
mod loader;
mod locale;
mod plural;
mod runtime;

pub use catalog::{Catalog, CatalogValue, Message, PluralForms, Segment};
pub use error::I18nError;
pub use loader::{CatalogLoadError, CatalogResource};
pub use locale::Locale;
pub use plural::{PluralCategory, plural_category};
pub use runtime::I18nVariant;
pub use runtime::{Argument, I18nBuilder, I18nStore, MissingArgumentPolicy, MissingKeyPolicy};
pub use silex_core::Rx;
pub use silex_core::reactivity::{
    Effect, Memo, ReadSignal, Resource, ResourceState, RwSignal, StoredValue, SuspenseContext,
};
pub use silex_core::traits::{RxGet, RxRead, RxWrite};
pub use silex_core::{RootHandle, Runtime, Scope};
#[cfg(feature = "persist")]
pub use silex_persist::Persistent;

#[cfg(feature = "browser")]
pub use browser::{
    TextDirection, detect_browser_locale, locale_direction, navigator_languages,
    resolve_requested_locale,
};

#[cfg(feature = "intl")]
pub use intl::{
    DateTimeFormat, DateTimeFormatter, Intl, IntlError, NumberFormat, NumberFormatter,
    format_date_time, format_number,
};

#[cfg(feature = "macros")]
pub use silex_i18n_macros::I18nKeys;

#[macro_export]
macro_rules! t {
    ($store:expr, $key:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        let __silex_i18n_store = $store;
        __silex_i18n_store.__memo(move || {
            let __silex_i18n_arguments = ::std::vec![
                $(
                    $crate::Argument::new(
                    stringify!($name),
                    ($value)?,
                    ),
                )*
            ];
            __silex_i18n_store.translate_now($key, &__silex_i18n_arguments)
        })
    }};
    ($store:expr, $variant:expr $(,)?) => {{
        let __silex_i18n_store = $store;
        __silex_i18n_store.__memo(move || {
            let __silex_i18n_variant = $variant;
            __silex_i18n_store.translate_variant_now(&__silex_i18n_variant)
        })
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::{ErrorReporter, ReactiveError, Runtime, Scope};
    use std::{cell::Cell, rc::Rc};

    fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
        scope.error_handler(|_| {}).expect("error handler")
    }

    fn locale(value: &str) -> Locale {
        Locale::new(value).expect("valid locale")
    }

    #[cfg(feature = "persist")]
    use std::cell::RefCell;

    #[cfg(feature = "persist")]
    use silex_persist::{
        BackendEventSink, BackendSubscribeError, BackendSubscription, PersistenceBackend,
        PersistenceError,
    };

    #[cfg(feature = "persist")]
    #[derive(Clone)]
    struct InputBackend {
        value: Rc<RefCell<Option<String>>>,
        get_calls: Rc<Cell<usize>>,
        set_calls: Rc<Cell<usize>>,
        subscribe_calls: Rc<Cell<usize>>,
        active_subscriptions: Rc<Cell<usize>>,
    }

    #[cfg(feature = "persist")]
    impl InputBackend {
        fn new() -> Self {
            Self {
                value: Rc::new(RefCell::new(None)),
                get_calls: Rc::new(Cell::new(0)),
                set_calls: Rc::new(Cell::new(0)),
                subscribe_calls: Rc::new(Cell::new(0)),
                active_subscriptions: Rc::new(Cell::new(0)),
            }
        }
    }

    #[cfg(feature = "persist")]
    impl<'scope> PersistenceBackend<'scope> for InputBackend {
        fn get(&self, _key: &str) -> Result<Option<String>, PersistenceError> {
            self.get_calls.set(self.get_calls.get() + 1);
            Ok(self.value.borrow().clone())
        }

        fn set(&self, _key: &str, value: &str) -> Result<(), PersistenceError> {
            self.set_calls.set(self.set_calls.get() + 1);
            *self.value.borrow_mut() = Some(value.to_string());
            Ok(())
        }

        fn remove(&self, _key: &str) -> Result<(), PersistenceError> {
            *self.value.borrow_mut() = None;
            Ok(())
        }

        fn subscribe(
            &self,
            _scope: Scope<'scope>,
            _key: impl Into<ref_str::LocalStaticRefStr>,
            _sink: BackendEventSink,
            _error_handler: ErrorReporter<'scope>,
        ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
            self.subscribe_calls.set(self.subscribe_calls.get() + 1);
            self.active_subscriptions
                .set(self.active_subscriptions.get() + 1);
            let active_subscriptions = self.active_subscriptions.clone();
            Ok(BackendSubscription::new(move || {
                active_subscriptions.set(active_subscriptions.get() - 1);
            }))
        }
    }

    #[test]
    fn normalizes_locale_and_builds_fallback_chain() {
        let locale = Locale::new("zh_hant_tw").expect("valid locale");
        assert_eq!(locale.as_str(), "zh-Hant-TW");
        assert_eq!(locale.language(), "zh");
        assert_eq!(
            locale
                .fallback_chain()
                .map(|value| value.to_string())
                .collect::<Vec<_>>(),
            vec!["zh-Hant-TW", "zh-Hant", "zh"]
        );
    }

    #[test]
    fn locale_constructor_reports_invalid_input() {
        let error = Locale::new("en US").expect_err("invalid locale must return an error");

        assert!(matches!(error, I18nError::InvalidLocale(_)));
    }

    #[test]
    fn translates_with_locale_fallback_and_interpolation() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let en = Catalog::from_entries(
                    locale("en-US"),
                    [("welcome.user", "Hello, {name}!"), ("only.en", "English")],
                )
                .expect("valid catalog");
                let zh =
                    Catalog::from_entries(locale("zh-CN"), [("welcome.user", "你好，{name}！")])
                        .expect("valid catalog");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("zh-CN"))
                    .fallback_locale(locale("en-US"))
                    .catalog(en)
                    .catalog(zh)
                    .build()
                    .expect("valid i18n store");
                let name = scope.rw_signal("Alice".to_string()).expect("name signal");
                let greeting = t!(store, "welcome.user", name = name.get()).expect("translation");
                assert_eq!(greeting.get().expect("translation value"), "你好，Alice！");
                name.set("Bob".to_string()).expect("name update");
                assert_eq!(greeting.get().expect("translation value"), "你好，Bob！");
                assert_eq!(
                    store.translate_now("only.en", &[]).expect("translation"),
                    "English"
                );

                store.set_locale(locale("en-GB")).expect("locale update");
                assert_eq!(greeting.get().expect("translation value"), "Hello, Bob!");
                store
                    .set_fallback_locale(locale("fr-FR"))
                    .expect("fallback update");
                assert_eq!(greeting.get().expect("translation value"), "welcome.user");
                store
                    .set_fallback_locale(locale("en-US"))
                    .expect("fallback update");
                assert_eq!(greeting.get().expect("translation value"), "Hello, Bob!");
            })
            .expect("child scope");
    }

    #[test]
    fn catalog_revision_invalidates_existing_translation_memo() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let initial = Catalog::from_entries(locale("en"), [("title", "Old")])
                    .expect("valid initial catalog");
                let replacement = Catalog::from_entries(locale("en"), [("title", "New")])
                    .expect("valid replacement catalog");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("en"))
                    .catalog(initial)
                    .build()
                    .expect("valid i18n store");
                let title = t!(store, "title").expect("translation");

                assert_eq!(title.get().expect("translation value"), "Old");
                store.insert_catalog(replacement).expect("catalog update");
                assert_eq!(title.get().expect("translation value"), "New");
                store
                    .remove_catalog(&locale("en"))
                    .expect("catalog removal");
                assert_eq!(title.get().expect("translation value"), "title");
            })
            .expect("child scope");
    }

    #[test]
    fn catalog_cache_updates_before_translation_memo_reruns() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let initial = Catalog::from_entries(locale("en"), [("title", "Old")])
                    .expect("valid initial catalog");
                let same = Catalog::from_entries(locale("en"), [("title", "Old")])
                    .expect("valid equal catalog");
                let replacement = Catalog::from_entries(locale("en"), [("title", "New")])
                    .expect("valid replacement catalog");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("en"))
                    .catalog(initial)
                    .build()
                    .expect("valid i18n store");
                let runs = Rc::new(Cell::new(0));
                let translation = store
                    .__memo({
                        let runs = runs.clone();
                        move || {
                            runs.set(runs.get() + 1);
                            store.translate_now("title", &[])
                        }
                    })
                    .expect("translation");

                assert_eq!(translation.get().expect("translation value"), "Old");
                assert_eq!(runs.get(), 1);

                store.insert_catalog(same).expect("catalog update");
                assert_eq!(translation.get().expect("translation value"), "Old");
                assert_eq!(runs.get(), 1);

                store.insert_catalog(replacement).expect("catalog update");
                assert_eq!(translation.get().expect("translation value"), "New");
                assert_eq!(runs.get(), 2);
            })
            .expect("child scope");
    }

    #[test]
    fn missing_key_and_argument_policies_are_independent() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let catalog = Catalog::from_entries(locale("en"), [("greeting", "Hi, {name}!")])
                    .expect("valid catalog");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("en"))
                    .catalog(catalog)
                    .missing_key(MissingKeyPolicy::Empty)
                    .missing_argument(MissingArgumentPolicy::Empty)
                    .build()
                    .expect("valid i18n store");

                assert_eq!(
                    store.translate_now("missing", &[]).expect("translation"),
                    ""
                );
                assert_eq!(
                    store.translate_now("greeting", &[]).expect("translation"),
                    "Hi, !"
                );
            })
            .expect("child scope");
    }

    #[test]
    fn reactivity_errors_keep_their_structured_variant() {
        let error = I18nError::from(ReactiveError::RuntimeMismatch);

        assert!(matches!(
            &error,
            I18nError::Reactivity(ReactiveError::RuntimeMismatch)
        ));
        assert!(error.to_string().contains("响应式节点属于不同的 Runtime"));
    }

    #[test]
    fn selects_plural_forms_and_keeps_missing_arguments() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let catalog = Catalog::from_entries(
                    locale("en"),
                    [(
                        "cart.items",
                        CatalogValue::plural([
                            ("one", "You have {count} item."),
                            ("other", "You have {count} items."),
                        ]),
                    )],
                )
                .expect("valid catalog");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("en"))
                    .catalog(catalog)
                    .build()
                    .expect("valid i18n store");

                assert_eq!(
                    store
                        .translate_now("cart.items", &[Argument::new("count", 1)])
                        .expect("translation"),
                    "You have 1 item."
                );
                assert_eq!(
                    store
                        .translate_now("cart.items", &[Argument::new("count", 2)])
                        .expect("translation"),
                    "You have 2 items."
                );
                assert_eq!(
                    store.translate_now("cart.items", &[]).expect("translation"),
                    "You have {count} items."
                );
            })
            .expect("child scope");
    }

    #[test]
    fn uses_the_fallback_catalog_locale_for_plural_rules() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let fallback = Catalog::from_entries(
                    locale("en"),
                    [(
                        "items",
                        CatalogValue::plural([("one", "one item"), ("other", "many items")]),
                    )],
                )
                .expect("valid catalog");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("zh-CN"))
                    .fallback_locale(locale("en"))
                    .catalog(fallback)
                    .build()
                    .expect("valid i18n store");

                assert_eq!(
                    store
                        .translate_now("items", &[Argument::new("count", 1)])
                        .expect("translation"),
                    "one item"
                );
            })
            .expect("child scope");
    }

    #[test]
    fn rejects_invalid_messages() {
        let error = Catalog::from_entries(locale("en"), [("bad", "Hello {name")])
            .expect_err("unclosed placeholder must fail");
        assert!(matches!(error, I18nError::InvalidMessage { .. }));

        let error = Catalog::from_entries(
            locale("en"),
            [("items", CatalogValue::plural([("one", "one")]))],
        )
        .expect_err("plural messages require other");
        assert!(matches!(error, I18nError::MissingOther { .. }));
    }

    #[cfg(all(feature = "persist", target_arch = "wasm32"))]
    #[test]
    fn locale_binding_takes_precedence_over_builder_locale() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let saved = Persistent::builder(scope, "silex-test-locale", test_handler(scope))
                    .local()
                    .parse::<Locale>()
                    .default(locale("en-US"))
                    .build()
                    .expect("locale binding");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("zh-CN"))
                    .locale_binding(saved)
                    .build()
                    .expect("valid i18n store");

                assert_eq!(
                    store.locale().get_untracked().expect("locale"),
                    locale("en-US")
                );
            })
            .expect("child scope");
    }

    #[cfg(feature = "persist")]
    #[test]
    fn locale_binding_stays_in_sync_inside_one_runtime() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let binding =
                    Persistent::builder(scope, "silex-memory-locale", test_handler(scope))
                        .backend(InputBackend::new())
                        .parse::<Locale>()
                        .default(locale("en-US"))
                        .build()
                        .expect("locale binding");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("zh-CN"))
                    .locale_binding(binding)
                    .build()
                    .expect("valid i18n store");

                assert_eq!(
                    store.locale().get_untracked().expect("locale"),
                    locale("en-US")
                );
                binding.set(locale("ja-JP")).expect("binding update");
                assert_eq!(
                    store.locale().get_untracked().expect("locale"),
                    locale("ja-JP")
                );
                store.set_locale(locale("de-DE")).expect("locale update");
                assert_eq!(
                    binding.get_untracked().expect("binding locale"),
                    locale("de-DE")
                );
            })
            .expect("child scope");
    }

    #[cfg(feature = "persist")]
    #[test]
    fn locale_binding_supports_root_and_child_scopes_in_one_runtime() {
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root scope");
        let root_backend = InputBackend::new();
        let child_backend = InputBackend::new();

        root.with_scope(|scope| {
            let root_binding = Persistent::builder(scope, "root-locale", test_handler(scope))
                .backend(root_backend.clone())
                .parse::<Locale>()
                .default(locale("en-US"))
                .build()
                .expect("root locale binding");
            let root_store = I18nBuilder::new(scope, test_handler(scope))
                .locale_binding(root_binding)
                .build()
                .expect("root binding should build");

            root_binding.set(locale("ja-JP")).expect("binding update");
            assert_eq!(
                root_store.locale().get_untracked().expect("locale"),
                locale("ja-JP")
            );

            scope
                .child(|child_scope| {
                    let child_binding =
                        Persistent::builder(child_scope, "child-locale", test_handler(child_scope))
                            .backend(child_backend.clone())
                            .parse::<Locale>()
                            .default(locale("en-US"))
                            .build()
                            .expect("child locale binding");
                    let child_store = I18nBuilder::new(child_scope, test_handler(child_scope))
                        .locale(locale("zh-CN"))
                        .locale_binding(child_binding)
                        .build()
                        .expect("child binding should build");

                    child_store
                        .set_locale(locale("ko-KR"))
                        .expect("locale update");
                    assert_eq!(
                        child_binding.get_untracked().expect("binding locale"),
                        locale("ko-KR")
                    );
                    assert_eq!(child_backend.subscribe_calls.get(), 1);
                })
                .expect("child scope");

            assert_eq!(root_backend.subscribe_calls.get(), 1);
            assert_eq!(root_backend.active_subscriptions.get(), 1);
            assert_eq!(child_backend.active_subscriptions.get(), 0);
        });

        root.dispose().expect("root cleanup");
        assert_eq!(root_backend.active_subscriptions.get(), 0);
    }

    #[cfg(feature = "persist")]
    #[test]
    fn locale_binding_suppresses_equal_bidirectional_writes() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let backend = InputBackend::new();
                let binding = Persistent::builder(scope, "equal-locale", test_handler(scope))
                    .backend(backend.clone())
                    .parse::<Locale>()
                    .default(locale("en-US"))
                    .build()
                    .expect("locale binding");
                let store = I18nBuilder::new(scope, test_handler(scope))
                    .locale(locale("en-US"))
                    .locale_binding(binding)
                    .build()
                    .expect("valid i18n store");
                let writes_after_build = backend.set_calls.get();

                binding.set(locale("en-US")).expect("binding update");
                store.set_locale(locale("en-US")).expect("locale update");

                assert_eq!(backend.set_calls.get(), writes_after_build);
                assert_eq!(backend.subscribe_calls.get(), 1);
            })
            .expect("child scope");
    }

    #[cfg(feature = "persist")]
    #[test]
    fn foreign_runtime_binding_fails_before_i18n_node_creation() {
        let mut foreign_runtime = Runtime::new();
        let foreign_root = foreign_runtime.run().expect("foreign root scope");
        let mut target_runtime = Runtime::new();
        let target_root = target_runtime.run().expect("target root scope");

        foreign_root.with_scope(|foreign_scope| {
            let backend = InputBackend::new();
            let binding = Persistent::builder(
                foreign_scope,
                "foreign-i18n-locale",
                test_handler(foreign_scope),
            )
            .backend(backend.clone())
            .parse::<Locale>()
            .default(locale("en-US"))
            .build()
            .expect("foreign locale binding");
            let before_backend = (
                backend.get_calls.get(),
                backend.set_calls.get(),
                backend.subscribe_calls.get(),
                backend.active_subscriptions.get(),
            );

            target_root.with_scope(|target_scope| {
                let handler = test_handler(target_scope);
                let before = target_scope.runtime_snapshot();
                let result = I18nBuilder::new(target_scope, handler)
                    .locale_binding(binding)
                    .build();

                assert!(matches!(
                    result,
                    Err(I18nError::Reactivity(ReactiveError::RuntimeMismatch))
                ));
                assert_eq!(target_scope.runtime_snapshot(), before);
                assert_eq!(
                    (
                        backend.get_calls.get(),
                        backend.set_calls.get(),
                        backend.subscribe_calls.get(),
                        backend.active_subscriptions.get(),
                    ),
                    before_backend
                );
            });

            assert_eq!(backend.active_subscriptions.get(), 1);
        });

        target_root.dispose().expect("target root cleanup");
        foreign_root.dispose().expect("foreign root cleanup");
    }

    #[cfg(feature = "json")]
    #[test]
    fn flattens_nested_json_and_rejects_path_collisions() {
        let catalog = Catalog::from_json(
            locale("en"),
            r#"{
                "home": { "title": "Silex" },
                "cart.items": { "one": "One item", "other": "{count} items" }
            }"#,
        )
        .expect("valid JSON catalog");
        assert_eq!(catalog.len(), 2);
        assert!(catalog.get("home.title").is_some());
        assert!(catalog.get("cart.items").is_some());

        let error = Catalog::from_json(
            locale("en"),
            r#"{ "home": "Silex", "home.title": "Title" }"#,
        )
        .expect_err("message/object collision must fail");
        assert!(matches!(error, I18nError::InvalidCatalog(_)));

        let error = Catalog::from_json(
            locale("en"),
            r#"{ "items": { "one": "one", "manyy": "many", "other": "other" } }"#,
        )
        .expect_err("unknown plural category must fail");
        assert!(matches!(error, I18nError::InvalidCatalog(_)));
    }
}
