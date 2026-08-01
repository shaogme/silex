use silex_i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json", crate = "silex_i18n")]
enum AppText {
    #[i18n(key = "home.title")]
    HomeTitle,
    #[i18n(key = "welcome.user")]
    WelcomeUser { name: String },
    #[i18n(key = "cart.items", count = "count")]
    CartItems { count: u32 },
}

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum DefaultPathText {
    #[i18n(key = "home.title")]
    HomeTitle,
}

fn accepts_i18n_variant<T: I18nVariant>() {}

fn main() {
    accepts_i18n_variant::<AppText>();
    accepts_i18n_variant::<DefaultPathText>();
    let _ = AppText::WelcomeUser {
        name: "Alice".to_string(),
    };
    let _ = AppText::CartItems { count: 2 };
}
