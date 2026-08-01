use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(
    path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json",
    crate = "missing_i18n_runtime"
)]
enum InvalidRuntimePath {
    #[i18n(key = "home.title")]
    HomeTitle,
}

fn main() {}
