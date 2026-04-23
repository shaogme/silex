use silex_core::traits::RxRead;
use silex_dom::prelude::*;
use silex_macros::component;
use std::rc::Rc;
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
pub fn Dynamic<V, FView>(#[standalone] view_fn: FView) -> impl View
where
    V: View + Clone + 'static,
    FView: RxRead<Value = V> + Clone + 'static,
{
    let view_fn = Rc::new(move || view_fn.with(|view| view.clone().into_any()));

    DynamicView { view_fn }
}

#[derive(Clone)]
struct DynamicView {
    view_fn: Rc<dyn Fn() -> AnyView + 'static>,
}

impl ApplyAttributes for DynamicView {}

impl View for DynamicView {
    fn mount(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        self.clone().mount_owned(parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>)
    where
        Self: Sized,
    {
        mount_dynamic_internal(self.view_fn, parent, attrs);
    }
}

fn mount_dynamic_internal(
    view_fn: Rc<dyn Fn() -> AnyView + 'static>,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) {
    use silex_dom::view::any::RenderThunk;
    silex_dom::view::mount_dynamic_view_universal(
        parent,
        attrs,
        RenderThunk::new(move |args| {
            let (p, a) = args;
            (view_fn.as_ref())().mount_owned(&p, a);
        }),
    );
}
