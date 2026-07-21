use silex_core::traits::RxRead;
use silex_dom::prelude::*;
use silex_macros::component;

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
pub fn Dynamic<V, FView>(view_fn: FView) -> impl View
where
    V: View + Clone + 'static,
    FView: RxRead<Value = V> + Clone + 'static,
{
    silex_core::rx! {
        view_fn.with(|view| view.clone().into_any())
    }
}
