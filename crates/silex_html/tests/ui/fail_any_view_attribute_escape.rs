use silex_view::elements::AnyView;
use silex_html::input;

fn main() {
    let _ = AnyView::new(input()).attr("value", "explicit");
}
