use silex_core::Runtime;
use silex_dom::view::{
    AnyView, DynamicRenderer, MountInstance, MountOwnerToken, View,
};

fn accept_root_view<'scope, V>(_: V)
where
    V: View<'scope> + 'scope,
{
}

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    {
        let owner = root.access();
        let _token = MountOwnerToken::new(owner);

        let borrowed_view = String::from("borrowed-view");
        let view: AnyView<'_> = AnyView::new(borrowed_view.as_str());
        accept_root_view(view);

        let _renderer: DynamicRenderer<'_> =
            DynamicRenderer::new(|_| Ok(MountInstance::from_nodes(Vec::new())));
    }

    root.close().expect("root close should succeed");
}
