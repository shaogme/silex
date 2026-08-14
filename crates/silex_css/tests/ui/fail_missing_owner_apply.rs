use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn apply<'scope, C>(ctx: C, element: &web_sys::Element)
where
    C: SilexContextProvider<'scope>,
{
    Style::new(ctx)
        .raw("--color", "red")
        .expect("style should build")
        .apply_to_element(element);
}

fn main() {}
