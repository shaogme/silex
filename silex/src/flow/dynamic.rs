use silex_core::traits::RxRead;
use silex_dom::prelude::*;
use silex_macros::component;
use web_sys::Node;

/// Dynamic 组件：用于渲染动态内容，类似于 SolidJS 的 <Dynamic>
///
/// 它接受一个返回 `View` 的闭包，并在该闭包依赖发生变化时自动刷新。
///
/// # 示例
///
/// ```rust
/// use silex::prelude::*;
///
/// let (component_name, set_component_name) = Signal::pair("A");
///
/// Dynamic(rx! {
///     let name = component_name.get();
///     if name == "A" {
///         "Component A"
///     } else {
///         "Component B"
///     }
/// });
/// ```
#[component]
pub fn Dynamic<V, FView>(view_fn: FView) -> impl Mount + MountRef
where
    V: MountRef + 'static,
    FView: RxRead<Value = V> + Clone + 'static,
{
    DynamicView {
        view_fn,
        _marker: std::marker::PhantomData,
    }
}

#[derive(Clone)]
struct DynamicView<'a, V, FView> {
    view_fn: Prop<'a, FView>,
    _marker: std::marker::PhantomData<V>,
}

impl<'a, V, FView> ApplyAttributes for DynamicView<'a, V, FView> {}

impl<'a, V, FView> Mount for DynamicView<'a, V, FView>
where
    V: MountRef + 'static,
    FView: RxRead<Value = V> + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_dynamic_internal(self.view_fn, parent, attrs);
    }
}

impl<'a, V, FView> MountRef for DynamicView<'a, V, FView>
where
    V: MountRef + 'static,
    FView: RxRead<Value = V> + Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_dynamic_internal(Prop::new_owned(self.view_fn.clone()), parent, attrs);
    }
}

fn mount_dynamic_internal<'a, V, FView>(
    view_fn: Prop<'a, FView>,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    V: MountRef + 'static,
    FView: RxRead<Value = V> + Clone + 'static,
{
    let view_fn = view_fn.into_owned();
    use silex_dom::view::any::RenderThunk;
    silex_dom::view::mount_dynamic_view_universal(
        parent,
        attrs,
        RenderThunk::new(move |args| {
            let (p, a) = args;
            view_fn.with(|view| view.mount_ref(&p, a));
        }),
    );
}
