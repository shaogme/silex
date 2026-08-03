use silex_core::Runtime;
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{AnyView, RenderThunk, ScopedViewOwner, ViewOwner};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let borrowed_view = String::from("borrowed-view");
        let _direct_view: AnyView<'_, '_> = AnyView::new(borrowed_view.as_str());
        let (read, _) = scope.signal(borrowed_view.as_str());
        let _view: AnyView<'_, '_> = AnyView::new(read);

        let owner = ScopedViewOwner::new(*scope);
        let _token = owner.token();

        let borrowed_attr = String::from("borrowed-attribute");
        let _attr: PendingAttribute<'_, '_> = PendingAttribute::new_listener(move |_| {
            let _ = borrowed_attr.as_str();
        });

        let borrowed_renderer = String::from("borrowed-renderer");
        let _renderer: RenderThunk<'_, '_> = RenderThunk::new(move |_| {
            let _ = borrowed_renderer.as_str();
        });
    });
}
