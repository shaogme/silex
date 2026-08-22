#![cfg(feature = "macros")]

use silex_core::{Runtime, RxGet};
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
    runtime
        .with_transient(|owner| {
            let catalog = Catalog::from_entries(
                Locale::new("en").expect("valid locale"),
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
            let handler = owner.error_handler(|_| {}).expect("error handler");
            let store = I18nBuilder::new(owner, handler.view())
                .locale(Locale::new("en").expect("valid locale"))
                .catalog(catalog)
                .build()
                .expect("valid store");
            let name = owner.signal("Alice".to_string()).expect("name signal");
            let greeting = t!(
                store,
                Text::WelcomeUser {
                    name: name.get().expect("name value")
                }
            )
            .expect("greeting translation");

            assert_eq!(greeting.get().expect("greeting value"), "Hello, Alice!");
            name.set("Bob".to_string()).expect("name update");
            assert_eq!(greeting.get().expect("greeting value"), "Hello, Bob!");
            assert_eq!(
                t!(store, Text::CartItems { count: 2 })
                    .expect("cart translation")
                    .get()
                    .expect("cart value"),
                "You have 2 items."
            );
        })
        .expect("child owner");
}

#[test]
fn typed_key_memo_tracks_fallback_and_catalog_revision() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let french = Catalog::from_entries(
                Locale::new("fr").expect("valid locale"),
                [("welcome.user", CatalogValue::from("Bonjour, {name}!"))],
            )
            .expect("valid french catalog");
            let german = Catalog::from_entries(
                Locale::new("de").expect("valid locale"),
                [("welcome.user", CatalogValue::from("Hallo, {name}!"))],
            )
            .expect("valid german catalog");
            let handler = owner.error_handler(|_| {}).expect("error handler");
            let store = I18nBuilder::new(owner, handler.view())
                .locale(Locale::new("es-MX").expect("valid locale"))
                .fallback_locale(Locale::new("fr").expect("valid locale"))
                .catalog(french)
                .catalog(german)
                .build()
                .expect("valid store");
            let name = owner.signal("Alice".to_string()).expect("name signal");
            let greeting = t!(
                store,
                Text::WelcomeUser {
                    name: name.get().expect("name value")
                }
            )
            .expect("greeting translation");

            assert_eq!(greeting.get().expect("greeting value"), "Bonjour, Alice!");
            name.set("Bob".to_string()).expect("name update");
            assert_eq!(greeting.get().expect("greeting value"), "Bonjour, Bob!");

            store
                .set_fallback_locale(Locale::new("de").expect("valid locale"))
                .expect("fallback update");
            assert_eq!(greeting.get().expect("greeting value"), "Hallo, Bob!");

            store
                .insert_catalog(
                    Catalog::from_entries(
                        Locale::new("de").expect("valid locale"),
                        [("welcome.user", CatalogValue::from("Guten Tag, {name}!"))],
                    )
                    .expect("valid replacement catalog"),
                )
                .expect("catalog insertion");
            assert_eq!(greeting.get().expect("greeting value"), "Guten Tag, Bob!");

            store
                .remove_catalog(&Locale::new("de").expect("valid locale"))
                .expect("catalog removal");
            assert_eq!(greeting.get().expect("greeting value"), "welcome.user");
            store
                .set_fallback_locale(Locale::new("fr").expect("valid locale"))
                .expect("fallback update");
            assert_eq!(greeting.get().expect("greeting value"), "Bonjour, Bob!");
        })
        .expect("child owner");
}
