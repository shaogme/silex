use silex_core::{reactivity::Effect, traits::Get};
use silex_dom::View;
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
/// let (component_name, set_component_name) = signal("A");
///
/// Dynamic::new(move || {
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
    V: View,
    F: Get<Value = V> + 'static,
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
    V: View + 'static,
{
    /// 创建一个 Dynamic 组件，该组件绑定到一个数据源 (Source)，
    /// 并使用映射函数将数据转换为视图。
    ///
    /// # 示例
    /// ```ignore
    /// Dynamic::bind(mode, |m| view_match!(m, { ... }))
    /// ```
    pub fn bind<S, T, Map>(source: S, map_fn: Map) -> Dynamic<V, impl Fn() -> V>
    where
        S: Get<Value = T> + 'static,
        Map: Fn(T) -> V + 'static,
        T: 'static,
        V: Clone,
    {
        let combined_accessor = move || map_fn(source.get());
        Dynamic::new(combined_accessor)
    }
}

impl<V, F> View for Dynamic<V, F>
where
    V: View,
    F: Get<Value = V> + 'static,
{
    fn mount(self, parent: &Node) {
        let document = silex_dom::document();

        // 1. Create Anchors
        let start_marker = document.create_comment("dyn-start");
        let start_node: Node = start_marker.into();

        if let Err(e) = parent
            .append_child(&start_node)
            .map_err(crate::SilexError::from)
        {
            silex_core::error::handle_error(e);
            return;
        }

        let end_marker = document.create_comment("dyn-end");
        let end_node: Node = end_marker.into();

        if let Err(e) = parent
            .append_child(&end_node)
            .map_err(crate::SilexError::from)
        {
            silex_core::error::handle_error(e);
            return;
        }

        let view_fn = self.view_fn;

        Effect::new(move |_| {
            let new_view = view_fn.get();

            // 清理旧内容
            if let Some(parent) = start_node.parent_node() {
                while let Some(sibling) = start_node.next_sibling() {
                    if sibling == end_node {
                        break;
                    }
                    let _ = parent.remove_child(&sibling);
                }
            }

            // 准备新内容
            let fragment = document.create_document_fragment();
            let fragment_node: Node = fragment.clone().into();
            new_view.mount(&fragment_node);

            // 插入新内容
            if let Some(parent) = end_node.parent_node() {
                let _ = parent.insert_before(&fragment_node, Some(&end_node));
            }
        });
    }
}
