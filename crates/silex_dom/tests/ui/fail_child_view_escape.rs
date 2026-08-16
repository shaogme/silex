use silex_core::Runtime;
use silex_dom::view::AnyView;

fn main() {
    let mut runtime = Runtime::new();
    let _view: AnyView<'static> =
        runtime.with_transient(|owner| AnyView::new(owner.signal(0i32).expect("signal").0));
}
