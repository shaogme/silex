use silex_i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum RawIdentifierText {
    #[i18n(key = "welcome.user")]
    WelcomeUser { r#name: String },
    #[i18n(key = "cart.items")]
    CartItems { r#count: u32 },
}

fn main() {
    let value = RawIdentifierText::WelcomeUser {
        r#name: "Alice".to_string(),
    };
    assert_eq!(value.arguments()[0].name(), "name");
    assert_eq!(value.arguments()[0].value(), "Alice");

    let value = RawIdentifierText::CartItems { r#count: 2 };
    assert_eq!(value.arguments()[0].name(), "count");
    assert_eq!(value.count_name(), Some("count"));
}
