use silex_view::elements::AnyView;
use silex_html::{FormAttributes, input};

fn main() {
    let _ = AnyView::new(input()).value("wrong");
}
