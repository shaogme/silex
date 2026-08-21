use silex_dom::{
    attribute::AttributeBuilder,
    view::AnyView,
};
use silex_html::input;

fn main() {
    let _ = AnyView::new(input()).attr("value", "explicit");
}
