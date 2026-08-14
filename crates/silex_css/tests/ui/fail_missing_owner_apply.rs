use silex_core::SilexContextProvider;
use silex_css::prelude::*;

fn apply<'scope, C>(context: C, element: &web_sys::Element)
where
    C: SilexContextProvider<'scope>,
{
    Style::new(context)
        .raw("--color", "red")
        .expect("style should build")
        .apply_to_element(element);
}

fn main() {}
