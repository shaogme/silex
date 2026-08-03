use silex_core::Runtime;
use silex_dom::view::AnyView;

fn main() {
    let mut runtime = Runtime::new();
    let _view: AnyView<'static, 'static> = runtime.child(|scope| {
        AnyView::new(scope.signal(0i32).0)
    });
}
