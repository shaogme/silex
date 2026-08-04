use silex_css::prelude::*;

fn apply(element: &web_sys::Element) {
    Style::new().raw("--color", "red").apply_to_element(element);
}

fn main() {
    let _ = apply;
}
