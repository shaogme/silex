use my_i18n::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "locales/en.json")]
pub enum RenamedText {
    #[i18n(key = "title")]
    Title,
}

#[test]
fn derive_resolves_a_renamed_direct_i18n_dependency() {
    use my_i18n::I18nVariant;

    let value = RenamedText::Title;

    assert_eq!(value.key(), "title");
    assert!(value.arguments().is_empty());
}
