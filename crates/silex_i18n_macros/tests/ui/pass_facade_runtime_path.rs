mod facade {
    pub mod i18n {
        pub use silex_i18n::*;
    }
}

use facade::i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(
    path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json",
    crate = "crate::facade::i18n"
)]
enum FacadeText {
    #[i18n(key = "home.title")]
    HomeTitle,
}

fn main() {
    let value = FacadeText::HomeTitle;
    assert_eq!(value.key(), "home.title");
}
