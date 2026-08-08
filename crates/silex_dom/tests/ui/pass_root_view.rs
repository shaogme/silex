use silex_core::Runtime;
use silex_dom::view::{AnyView, RenderThunk, ScopedViewOwner, View, ViewOwner};

fn accept_root_view<'scope, V>(_: V)
where
    V: View<'scope> + 'scope,
{
}

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let owner = ScopedViewOwner::new(scope, scope.error_handler(|_| {}));
        let _token = owner.token();

        let borrowed_view = String::from("borrowed-view");
        let view: AnyView<'_> = AnyView::new(borrowed_view.as_str());
        accept_root_view(view);

        let _renderer: RenderThunk<'_> = RenderThunk::new(|_| Ok(()));
    }

    root.dispose().expect("root disposal should succeed");
}
