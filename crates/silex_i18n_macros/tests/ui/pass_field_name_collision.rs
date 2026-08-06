use silex_i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum ReservedFieldText {
    #[i18n(key = "reserved.argument")]
    Reserved {
        __silex_i18n_arguments: String,
    },
}

fn main() {
    let value = ReservedFieldText::Reserved {
        __silex_i18n_arguments: "Alice".to_string(),
    };
    assert_eq!(value.arguments()[0].name(), "__silex_i18n_arguments");
    assert_eq!(value.arguments()[0].value(), "Alice");
}
