#![cfg(feature = "macros")]

use silex_core::Runtime;
use silex_i18n::{Catalog, CatalogValue, I18nBuilder, I18nKeys, Locale, t};

#[derive(I18nKeys)]
#[i18n(path = "tests/fixtures/typed-en.json", crate = "silex_i18n")]
enum Text {
    #[i18n(key = "welcome.user")]
    WelcomeUser { name: String },
    #[i18n(key = "cart.items")]
    CartItems { count: u32 },
}

#[test]
fn typed_key_macro_keeps_reactive_arguments_inside_the_memo() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let catalog = Catalog::from_entries(
            Locale::new("en"),
            [
                ("welcome.user", CatalogValue::from("Hello, {name}!")),
                (
                    "cart.items",
                    CatalogValue::plural([
                        ("one", "You have {count} item."),
                        ("other", "You have {count} items."),
                    ]),
                ),
            ],
        )
        .expect("valid catalog");
        let store = I18nBuilder::new(scope)
            .locale(Locale::new("en"))
            .catalog(catalog)
            .build()
            .expect("valid store");
        let name = scope.rw_signal("Alice".to_string());
        let greeting = t!(store, Text::WelcomeUser { name: name.get() });

        assert_eq!(greeting.get(), "Hello, Alice!");
        name.set("Bob".to_string());
        assert_eq!(greeting.get(), "Hello, Bob!");
        assert_eq!(
            t!(store, Text::CartItems { count: 2 }).get(),
            "You have 2 items."
        );
    });
}

#[test]
fn typed_key_memo_tracks_fallback_and_catalog_revision() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let french = Catalog::from_entries(
            Locale::new("fr"),
            [("welcome.user", CatalogValue::from("Bonjour, {name}!"))],
        )
        .expect("valid french catalog");
        let german = Catalog::from_entries(
            Locale::new("de"),
            [("welcome.user", CatalogValue::from("Hallo, {name}!"))],
        )
        .expect("valid german catalog");
        let store = I18nBuilder::new(scope)
            .locale(Locale::new("es-MX"))
            .fallback_locale(Locale::new("fr"))
            .catalog(french)
            .catalog(german)
            .build()
            .expect("valid store");
        let name = scope.rw_signal("Alice".to_string());
        let greeting = t!(store, Text::WelcomeUser { name: name.get() });

        assert_eq!(greeting.get(), "Bonjour, Alice!");
        name.set("Bob".to_string());
        assert_eq!(greeting.get(), "Bonjour, Bob!");

        store.set_fallback_locale(Locale::new("de"));
        assert_eq!(greeting.get(), "Hallo, Bob!");

        store.insert_catalog(
            Catalog::from_entries(
                Locale::new("de"),
                [("welcome.user", CatalogValue::from("Guten Tag, {name}!"))],
            )
            .expect("valid replacement catalog"),
        );
        assert_eq!(greeting.get(), "Guten Tag, Bob!");

        store.remove_catalog(&Locale::new("de"));
        assert_eq!(greeting.get(), "welcome.user");
        store.set_fallback_locale(Locale::new("fr"));
        assert_eq!(greeting.get(), "Bonjour, Bob!");
    });
}
