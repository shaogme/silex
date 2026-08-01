#![doc = "Silex internationalization runtime."]

mod catalog;
mod error;
mod locale;
mod plural;
mod runtime;

pub use catalog::{Catalog, CatalogValue, Message, PluralForms, Segment};
pub use error::I18nError;
pub use locale::Locale;
pub use plural::{PluralCategory, plural_category};
pub use runtime::{Argument, I18nBuilder, I18nStore, MissingArgumentPolicy, MissingKeyPolicy};
pub use silex_core::reactivity::{Memo, ReadSignal, RwSignal};
pub use silex_core::traits::{RxGet, RxRead, RxWrite};

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
