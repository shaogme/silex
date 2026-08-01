use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "../../../../crates/silex_i18n_macros/tests/fixtures/en-US.json", crate = "silex_i18n")]
enum InvalidPluralCount {
    #[i18n(key = "cart.items", count = "count")]
    CartItems { count: String },
}

fn main() {}
