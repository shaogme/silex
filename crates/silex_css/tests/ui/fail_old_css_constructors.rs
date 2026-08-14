use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn check<'scope, C>(ctx: C)
where
    C: SilexContextProvider<'scope>,
{
    let style = Style::new(ctx)
        .raw("--color", "red")
        .expect("style should build");
    let _ = style.into_rx();
}

fn main() {}
