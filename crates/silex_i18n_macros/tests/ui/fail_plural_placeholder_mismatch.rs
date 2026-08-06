use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(
    path = "../../../../crates/silex_i18n_macros/tests/fixtures/plural-mismatch.json",
    crate = "silex_i18n"
)]
enum InvalidPluralPlaceholders {
    #[i18n(key = "cart.items")]
    CartItems { count: u32 },
}

fn main() {}
