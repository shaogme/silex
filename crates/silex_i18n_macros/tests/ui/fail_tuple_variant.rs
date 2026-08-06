use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json", crate = "silex_i18n")]
enum InvalidTupleVariant {
    #[i18n(key = "welcome.user")]
    WelcomeUser(String),
}

fn main() {}
