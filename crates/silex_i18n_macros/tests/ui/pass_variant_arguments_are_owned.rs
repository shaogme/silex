use silex_i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum OwnedText {
    #[i18n(key = "welcome.user")]
    WelcomeUser { name: String },
}

fn main() {
    let value = OwnedText::WelcomeUser {
        name: "Alice".to_string(),
    };
    let arguments = value.arguments();
    assert_eq!(arguments[0].name(), "name");
    assert_eq!(arguments[0].value(), "Alice");
}
