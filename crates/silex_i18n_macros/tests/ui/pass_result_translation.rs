use silex_i18n::{Catalog, I18nBuilder, Locale, Runtime, RxGet, t};
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum Text {
    #[i18n(key = "home.title")]
    HomeTitle,
}

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let catalog = Catalog::from_entries(
                Locale::new("en-US").expect("valid locale"),
                [("home.title", "Home")],
            )
            .expect("valid catalog");
            let handler = owner.error_handler(|_| {}).expect("error handler");
            let store = I18nBuilder::new(owner, handler.view())
                .locale(Locale::new("en-US").expect("valid locale"))
                .catalog(catalog)
                .build()
                .expect("valid store");
            let translation = t!(store, Text::HomeTitle).expect("translation");

            assert_eq!(translation.get().expect("translation value"), "Home");
        })
        .expect("transient owner");
}
