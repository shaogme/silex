use silex_view::attribute::AttributeBuilder;
use silex_html::input;

fn main() {
    let _ = input().into_untyped().attr("value", "explicit");
}
