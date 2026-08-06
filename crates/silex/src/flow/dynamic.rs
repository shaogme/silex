use silex_core::{Scope, reactivity::ReactiveSource};
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
/// let (component_name, set_component_name) = scope.signal("A");
///
/// Dynamic(scope, rx!(scope; {
///     let name = component_name.get();
///     if name == "A" {
///         "Component A"
///     } else {
///         "Component B"
///     }
/// }));
/// ```
#[component]
pub fn Dynamic<'scope, V, FView>(scope: Scope<'scope>, view_fn: FView) -> impl View<'scope>
where
    V: View<'scope> + Clone + 'scope,
    FView: ReactiveSource<'scope, Value = V> + Clone + 'scope,
{
    let view_fn = scope.promote(view_fn);
    silex_core::rx!(scope; (*$view_fn).clone().into_any())
}
