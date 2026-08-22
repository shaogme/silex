use silex_core::{Runtime, RxGet, RxRead, SilexContext, SilexResult};
use std::error::Error;

#[derive(Clone, Copy)]
struct Settings<'scope> {
    theme: silex_core::Signal<'scope, String>,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| -> SilexResult<()> {
            let reporter = owner.error_handler(|_| {})?;
            let ctx = SilexContext::new(owner, reporter.view());
            let count = owner.signal(1_i32)?;

            let doubled = silex_rx::rx!(silex_core; @ctx ctx; $count * 2)?;
            assert_eq!(doubled.get()?, 2);

            count.set(3)?;
            assert_eq!(doubled.get()?, 6);

            let theme = owner.signal("light".to_owned())?;
            let settings = Settings { theme };
            let label = silex_core::rx!(ctx; format!("Theme: {}", $(settings.theme)))?;
            assert_eq!(label.get()?, "Theme: light");

            theme.set("dark".to_owned())?;
            assert_eq!(label.get()?, "Theme: dark");

            Ok(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}
