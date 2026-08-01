#![cfg(feature = "macros")]

use silex_core::reactivity::create_scope;
use silex_i18n::{
    Catalog, CatalogValue, I18nBuilder, I18nKeys, Locale, RwSignal, RxGet, RxWrite, t,
};

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
    create_scope(|| {
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
        let store = I18nBuilder::new()
            .locale(Locale::new("en"))
            .catalog(catalog)
            .build()
            .expect("valid store");
        let name = RwSignal::new("Alice".to_string());
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
