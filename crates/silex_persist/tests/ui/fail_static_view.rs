use silex_dom::view::View;
use silex_persist::Persistent;

fn static_view<'scope>(binding: Persistent<'scope, String>) -> Box<dyn View<'static>> {
    Box::new(binding)
}

fn main() {
    let _ = static_view;
}
