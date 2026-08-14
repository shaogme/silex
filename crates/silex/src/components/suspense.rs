use silex_core::reactivity::SuspenseContext;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::component;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SuspenseMode {
    #[default]
    KeepAlive,
    Unmount,
}

/// Suspense 组件
///
/// 用于处理异步加载状态。它会创建一个 SuspenseContext 并将其提供给 `children` 闭包。
/// 任何在 `children` 闭包内部创建的 Resource 都会自动注册到该上下文中。
///
/// # 示例
/// ```rust,ignore
/// Suspense(scope, move |ctx| {
///     let res = Resource::new(scope, id, fetch_user, Some(ctx));
///     div![
///         "User: ",
///         rx!(scope; res.get_data().unwrap_or_default())
///     ]
/// })
/// .fallback(div("Loading..."))
/// .build()
/// ```
#[component]
pub fn Suspense<'scope, Ctx, CH, R>(
    #[ctx] ctx: Ctx,
    children: CH,
    #[chain(default = AnyView::Empty)] fallback: AnyView<'scope>,
    #[chain(default)] mode: SuspenseMode,
) -> impl View<'scope>
where
    CH: Fn(SuspenseContext<'scope>) -> R + Clone + 'scope,
    R: View<'scope> + 'scope,
{
    let children = Rc::new(move |cx: SuspenseContext<'scope>| children(cx).into_any());

    // 创建属于此 Suspense 边界的上下文
    let context = SuspenseContext::new(scope)?;

    // 在组件初始化时（稳定作用域）执行一次工厂闭包。
    // 确保 Resource 实例绑定到稳定的组件作用域。
    let initial_view = children(context);

    match mode {
        SuspenseMode::KeepAlive => {
            let count = context.count;
            let content_display = silex_core::rx!(ctx; if *$count > 0 {
                "display: none".to_string()
            } else {
                "display: block".to_string()
            });
            let fallback_display = silex_core::rx!(ctx; if *$count > 0 {
                "display: block".to_string()
            } else {
                "display: none".to_string()
            });
            Ok(chain!(
                div(initial_view.clone())
                    .class("suspense-content")
                    .style(content_display),
                div(fallback.clone())
                    .class("suspense-fallback")
                    .style(fallback_display)
            )
            .into_any())
        }
        SuspenseMode::Unmount => {
            let count = context.count;
            let (is_first, set_is_first) = scope.signal(true)?;
            let initial_view = initial_view.clone();
            let children = children.clone();
            let fallback = fallback.clone();
            let content = silex_core::rx!(ctx; {
                if *$count == 0 {
                    if *$is_first {
                        set_is_first.set(false)?;
                        initial_view.clone()
                    } else {
                        children(context)
                    }
                } else {
                    AnyView::Empty
                }
            });
            let fallback_view = silex_core::rx!(ctx; {
                if *$count > 0 {
                    fallback.clone()
                } else {
                    AnyView::Empty
                }
            });
            Ok(chain!(content, fallback_view).into_any())
        }
    }
}
