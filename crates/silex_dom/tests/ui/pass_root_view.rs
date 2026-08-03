use silex_core::Runtime;
use silex_dom::view::{AnyView, RenderThunk, RootViewOwner, View, ViewOwner};

fn accept_root_view<V>(_: V)
where
    V: View<'static> + 'static,
{
}

fn main() {
    let mut runtime = Runtime::new();
    let root = runtime.run(|scope| {
        let owner = RootViewOwner::new(scope.clone());
        let _token = owner.token();

        let view = AnyView::new(String::from("owned-view"));
        accept_root_view(view);

        let _renderer: RenderThunk<'static> = RenderThunk::new(|_| {});
    });

    drop(root);
}
