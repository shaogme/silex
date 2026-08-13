use silex_core::Runtime;
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{AnyView, DynamicRenderer, MountInstance, MountOwner, ScopedMountOwner};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let borrowed_view = String::from("borrowed-view");
            let _direct_view: AnyView<'_> = AnyView::new(borrowed_view.as_str());
            let (read, _) = scope.signal(borrowed_view.as_str()).expect("signal");
            let _view: AnyView<'_> = AnyView::new(read);

            let owner = ScopedMountOwner::new(scope);
            let _token = owner.token();

            let borrowed_attr = String::from("borrowed-attribute");
            let _attr: PendingAttribute<'_> = PendingAttribute::new_listener(move |_| {
                let _ = borrowed_attr.as_str();
                Ok(())
            });

            let borrowed_renderer = String::from("borrowed-renderer");
            let _renderer: DynamicRenderer<'_> = DynamicRenderer::new(move |_| {
                let _ = borrowed_renderer.as_str();
                Ok(MountInstance::from_nodes(Vec::new()))
            });
        })
        .expect("child scope should initialize");
}
