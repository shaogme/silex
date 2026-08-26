use silex_view::attributes::AttributeBuilder;
use silex_html::input;

fn main() {
    let _ = input().into_untyped().attr("value", "explicit");
}
