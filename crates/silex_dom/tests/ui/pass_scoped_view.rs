use silex_core::Runtime;
use silex_dom::attribute::AttrOp;
use silex_dom::view::{AnyView, DynamicRenderer, MountInstance, MountOwnerToken};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let borrowed_view = String::from("borrowed-view");
            let _direct_view: AnyView<'_> = AnyView::new(borrowed_view.as_str());
            let read = owner.signal(borrowed_view.as_str()).expect("signal");
            let _view: AnyView<'_> = AnyView::new(read);

            let _token = MountOwnerToken::new(owner);

            let borrowed_attr = String::from("borrowed-attribute");
            let _attr: AttrOp<'_> = AttrOp::new_listener(move |_| {
                let _ = borrowed_attr.as_str();
                Ok(())
            });

            let borrowed_renderer = String::from("borrowed-renderer");
            let _renderer: DynamicRenderer<'_> = DynamicRenderer::new(move |_| {
                let _ = borrowed_renderer.as_str();
                Ok(MountInstance::from_nodes(Vec::new()))
            });
        })
        .expect("transient owner should initialize");
}
