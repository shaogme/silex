use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(context: C)
where
    C: SilexContextProvider<'scope>,
{
    let style = Style::new(context)
        .raw("--color", "red")
        .expect("style should build");
    let _ = style.into_rx();
}

fn main() {}
