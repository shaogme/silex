use silex_dom::view::AnyView;
use silex_html::input;

fn main() {
    let _ = AnyView::new(input()).attr("value", "explicit");
}
