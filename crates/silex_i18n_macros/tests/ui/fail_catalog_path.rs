use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/does-not-exist.json")]
enum InvalidCatalogPath {
    #[i18n(key = "home.title")]
    HomeTitle,
}

fn main() {}
