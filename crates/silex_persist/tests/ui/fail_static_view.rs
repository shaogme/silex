use silex_persist::Persistent;
use silex_view::View;

fn static_view<'scope>(binding: Persistent<'scope, String>) -> Box<dyn View<'static>> {
    Box::new(binding)
}

fn main() {
    let _ = static_view;
}
