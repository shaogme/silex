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
pub use silex_core::reactivity::{
    Memo, ReadSignal, Resource, ResourceState, RwSignal, SuspenseContext,
};
pub use silex_core::traits::{RxGet, RxRead, RxWrite};
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
        $crate::Memo::new(move |_| {
            let mut __silex_i18n_arguments = ::std::vec::Vec::new();
            $(
                __silex_i18n_arguments.push($crate::Argument::new(
                    stringify!($name),
                    ($value),
                ));
            )*
            __silex_i18n_store.translate_now($key, &__silex_i18n_arguments)
        })
    }};
    ($store:expr, $variant:expr $(,)?) => {{
        let __silex_i18n_store = $store;
        $crate::Memo::new(move |_| {
            let __silex_i18n_variant = $variant;
            __silex_i18n_store.translate_variant_now(&__silex_i18n_variant)
        })
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::reactivity::create_scope;

    #[test]
    fn normalizes_locale_and_builds_fallback_chain() {
        let locale = Locale::parse("zh_hant_tw").expect("valid locale");
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
    fn translates_with_locale_fallback_and_interpolation() {
        create_scope(|| {
            let en = Catalog::from_entries(
                Locale::new("en-US"),
                [("welcome.user", "Hello, {name}!"), ("only.en", "English")],
            )
            .expect("valid catalog");
            let zh =
                Catalog::from_entries(Locale::new("zh-CN"), [("welcome.user", "你好，{name}！")])
                    .expect("valid catalog");
            let store = I18nBuilder::new()
                .locale(Locale::new("zh-CN"))
                .fallback_locale(Locale::new("en-US"))
                .catalog(en)
                .catalog(zh)
                .build()
                .expect("valid i18n store");
            let name = RwSignal::new("Alice".to_string());
            let greeting = t!(store, "welcome.user", name = name.get());
            assert_eq!(greeting.get(), "你好，Alice！");
            name.set("Bob".to_string());
            assert_eq!(greeting.get(), "你好，Bob！");
            assert_eq!(store.translate_now("only.en", &[]), "English");

            store.set_locale(Locale::new("en-GB"));
            assert_eq!(greeting.get(), "Hello, Bob!");
        });
    }

    #[test]
    fn selects_plural_forms_and_keeps_missing_arguments() {
        create_scope(|| {
            let catalog = Catalog::from_entries(
                Locale::new("en"),
                [(
                    "cart.items",
                    CatalogValue::plural([
                        ("one", "You have {count} item."),
                        ("other", "You have {count} items."),
                    ]),
                )],
            )
            .expect("valid catalog");
            let store = I18nBuilder::new()
                .locale(Locale::new("en"))
                .catalog(catalog)
                .build()
                .expect("valid i18n store");

            assert_eq!(
                store.translate_now("cart.items", &[Argument::new("count", 1)]),
                "You have 1 item."
            );
            assert_eq!(
                store.translate_now("cart.items", &[Argument::new("count", 2)]),
                "You have 2 items."
            );
            assert_eq!(
                store.translate_now("cart.items", &[]),
                "You have {count} items."
            );
        });
    }

    #[test]
    fn uses_the_fallback_catalog_locale_for_plural_rules() {
        create_scope(|| {
            let fallback = Catalog::from_entries(
                Locale::new("en"),
                [(
                    "items",
                    CatalogValue::plural([("one", "one item"), ("other", "many items")]),
                )],
            )
            .expect("valid catalog");
            let store = I18nBuilder::new()
                .locale(Locale::new("zh-CN"))
                .fallback_locale(Locale::new("en"))
                .catalog(fallback)
                .build()
                .expect("valid i18n store");

            assert_eq!(
                store.translate_now("items", &[Argument::new("count", 1)]),
                "one item"
            );
        });
    }

    #[test]
    fn rejects_invalid_messages() {
        let error = Catalog::from_entries(Locale::new("en"), [("bad", "Hello {name")])
            .expect_err("unclosed placeholder must fail");
        assert!(matches!(error, I18nError::InvalidMessage { .. }));

        let error = Catalog::from_entries(
            Locale::new("en"),
            [("items", CatalogValue::plural([("one", "one")]))],
        )
        .expect_err("plural messages require other");
        assert!(matches!(error, I18nError::MissingOther { .. }));
    }

    #[cfg(all(feature = "persist", target_arch = "wasm32"))]
    #[test]
    fn locale_binding_takes_precedence_over_builder_locale() {
        create_scope(|| {
            let saved = Persistent::builder("silex-test-locale")
                .local()
                .parse::<Locale>()
                .default(Locale::new("en-US"))
                .build();
            let store = I18nBuilder::new()
                .locale(Locale::new("zh-CN"))
                .locale_binding(saved)
                .build()
                .expect("valid i18n store");

            assert_eq!(store.locale().get_untracked(), Locale::new("en-US"));
        });
    }

    #[cfg(feature = "json")]
    #[test]
    fn flattens_nested_json_and_rejects_path_collisions() {
        let catalog = Catalog::from_json(
            Locale::new("en"),
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
            Locale::new("en"),
            r#"{ "home": "Silex", "home.title": "Title" }"#,
        )
        .expect_err("message/object collision must fail");
        assert!(matches!(error, I18nError::InvalidCatalog(_)));

        let error = Catalog::from_json(
            Locale::new("en"),
            r#"{ "items": { "one": "one", "manyy": "many", "other": "other" } }"#,
        )
        .expect_err("unknown plural category must fail");
        assert!(matches!(error, I18nError::InvalidCatalog(_)));
    }
}
