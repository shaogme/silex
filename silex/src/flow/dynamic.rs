use silex_core::traits::RxRead;
use silex_dom::prelude::{ApplyAttributes, Mount, MountRef};
use web_sys::Node;

/// Dynamic 组件：用于渲染动态内容，类似于 SolidJS 的 <Dynamic>
///
/// 它接受一个返回 `View` 的闭包，并在该闭包的依赖发生变化时自动重新渲染。
/// 通常用于根据状态动态切换组件。
///
/// # 示例
///
/// ```rust
/// use silex::prelude::*;
///
/// let (component_name, set_component_name) = Signal::pair("A");
///
/// Dynamic::new(rx! {
///     let name = component_name.get();
///     if name == "A" {
///         "Component A"
///     } else {
///         "Component B"
///     }
/// });
/// ```
#[derive(Clone)]
pub struct Dynamic<V, F> {
    view_fn: F,
    _marker: std::marker::PhantomData<V>,
}

impl<V, F> Dynamic<V, F>
where
    V: MountRef + 'static,
    F: RxRead<Value = V> + 'static,
{
    pub fn new(f: F) -> Self {
        Self {
            view_fn: f,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<V> Dynamic<V, ()>
where
    V: MountRef + 'static,
{
    /// 创建一个 Dynamic 组件，该组件绑定到一个数据源 (Source)，
    /// 并使用映射函数将数据转换为视图。
    ///
    /// # 示例
    /// ```ignore
    /// Dynamic::bind(mode, |m| view_match!(m, { ... }))
    /// ```
    pub fn bind<S, T, Map>(
        source: S,
        map_fn: Map,
    ) -> Dynamic<V, silex_core::Rx<V, silex_core::RxValueKind>>
    where
        S: RxRead<Value = T> + 'static,
        Map: Fn(T) -> V + 'static,
        T: Clone + 'static,
        V: MountRef + 'static,
    {
        let combined_accessor = silex_core::rx!(source.with(|val| map_fn(val.clone())));
        Dynamic::new(combined_accessor)
    }
}

impl<V, F> ApplyAttributes for Dynamic<V, F>
where
    V: MountRef + 'static,
    F: RxRead<Value = V> + Clone + 'static,
{
}

impl<V, F> Mount for Dynamic<V, F>
where
    V: MountRef + 'static,
    F: RxRead<Value = V> + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_dynamic_internal(self.view_fn, parent, attrs);
    }
}

impl<V, F> MountRef for Dynamic<V, F>
where
    V: MountRef + 'static,
    F: RxRead<Value = V> + Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_dynamic_internal(self.view_fn.clone(), parent, attrs);
    }
}

fn mount_dynamic_internal<V, F>(
    view_fn: F,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    V: MountRef + 'static,
    F: RxRead<Value = V> + 'static,
{
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
