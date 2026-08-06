use silex_i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum GenericText<T>
where
    T: std::fmt::Display,
{
    #[i18n(key = "welcome.user")]
    WelcomeUser { name: T },
}

fn accepts_i18n_variant<T: I18nVariant>() {}

fn main() {
    accepts_i18n_variant::<GenericText<String>>();
    let value = GenericText::WelcomeUser {
        name: "Alice".to_string(),
    };
    assert_eq!(value.arguments()[0].value(), "Alice");
}
