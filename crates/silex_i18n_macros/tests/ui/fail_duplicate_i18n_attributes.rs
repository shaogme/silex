use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(
    path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json",
    path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json"
)]
enum DuplicateContainerAttribute {
    #[i18n(key = "home.title")]
    HomeTitle,
}

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum DuplicateVariantKey {
    #[i18n(key = "home.title", key = "home.title")]
    HomeTitle,
}

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json")]
enum DuplicateVariantCount {
    #[i18n(key = "cart.items", count = "count", count = "count")]
    CartItems { count: u32 },
}

fn main() {}
