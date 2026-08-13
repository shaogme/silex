use silex_core::Runtime;
use silex_dom::view::{
    AnyView, DynamicRenderer, MountInstance, MountOwner, ScopedMountOwner, View,
};

fn accept_root_view<'scope, V>(_: V)
where
    V: View<'scope> + 'scope,
{
}

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    {
        let scope = root.scope();
        let owner = ScopedMountOwner::new(scope);
        let _token = owner.token();

        let borrowed_view = String::from("borrowed-view");
        let view: AnyView<'_> = AnyView::new(borrowed_view.as_str());
        accept_root_view(view);

        let _renderer: DynamicRenderer<'_> =
            DynamicRenderer::new(|_| Ok(MountInstance::from_nodes(Vec::new())));
    }

    root.dispose().expect("root disposal should succeed");
}
