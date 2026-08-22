use silex_core::{ErrorHandlerToken, OwnerAccess, Runtime, RxGet, SilexResult};
use silex_i18n::{Argument, Catalog, CatalogValue, I18nBuilder, Locale, t};
use std::error::Error;

fn handler<'scope>(owner: OwnerAccess<'scope>) -> SilexResult<ErrorHandlerToken<'scope>> {
    owner.error_handler(|_| {})
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let catalog = Catalog::from_entries(
                Locale::new("en-US")?,
                [
                    ("welcome.user", CatalogValue::from("Hello, {name}!")),
                    (
                        "cart.items",
                        CatalogValue::plural([
                            ("one", "You have {count} item."),
                            ("other", "You have {count} items."),
                        ]),
                    ),
                ],
            )?;
            let error_handler = handler(owner)?;
            let store = I18nBuilder::new(owner, error_handler.view())
                .locale(Locale::new("en-US")?)
                .catalog(catalog)
                .build()?;
            let name = owner.rw_signal(String::from("Alice"))?;
            let greeting = t!(store, "welcome.user", name = name.get())?;

            assert_eq!(greeting.get()?, "Hello, Alice!");
            name.set(String::from("Bob"))?;
            assert_eq!(greeting.get()?, "Hello, Bob!");
            assert_eq!(
                store.translate_now("cart.items", &[Argument::new("count", 2)])?,
                "You have 2 items."
            );
            Ok::<(), silex_core::SilexError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}
