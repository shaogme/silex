use crate::dom::view::AnyView;
use crate::reactivity::{create_memo, provide_context, use_context};
use crate::router::context::{ViewFactory, use_router};
use std::rc::Rc;

/// 路由深度上下文，用于指示当前 Outlet 处于第几层路由
#[derive(Clone, Copy, Debug, PartialEq)]
struct RouteDepth(usize);

/// Outlet 组件：渲染匹配到的下一级路由视图
#[allow(non_snake_case)]
pub fn Outlet() -> ViewFactory {
    // 1. 获取当前深度 (默认为 0)
    // 注意: use_context 在组件初始化时必须同步调用。
    // Outlet 本身应该是一个组件（只执行一次）。
    let depth = use_context::<RouteDepth>().unwrap_or(RouteDepth(0)).0;

    // 2. 获取 Router Context
    let router = use_router().expect("<Outlet /> must be used inside a <Router>");

    // 3. 创建 Memo 仅监听当前深度的匹配结果
    let matched_factory = create_memo(move || {
        let matches = router.matches.get();
        matches.get(depth).map(|m| m.view_factory.clone())
    });

    // 4. 返回 ViewFactory (由 context.rs 实现了 View 特征)
    // 这里的闭包将被 ViewFactory::mount 里的 closure 调用，从而进入 create_effect
    ViewFactory(Rc::new(move || {
        if let Some(factory_wrapper) = matched_factory.get() {
            // 为下级路由提供深度 Context
            // 注意：这里是在 create_effect 内部并在渲染子组件前调用 provide_context
            // 这是合法的，因为子组件会在稍后 mount 时调用 use_context
            let _ = provide_context(RouteDepth(depth + 1));
            // 调用工厂函数创建视图
            (factory_wrapper.0)()
        } else {
            // 没有匹配到下一级路由，渲染空
            AnyView::new(())
        }
    }))
}
