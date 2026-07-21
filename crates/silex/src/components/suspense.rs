use silex_core::reactivity::{Signal, SuspenseContext};
use silex_core::traits::{RxGet, RxWrite};
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, render};
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
/// Suspense(move || {
///     let res = Resource::new(id, fetch_user);
///     div![
///         "User: ",
///         rx!(res.get().map(|u| u.name))
///     ]
/// })
/// .fallback(div("Loading..."))
/// ```
#[component]
pub fn Suspense<CH, R>(
    children: CH,
    #[chain(default = AnyView::Empty)] fallback: AnyView,
    #[chain(default)] mode: SuspenseMode,
) -> impl View
where
    CH: Fn(SuspenseContext) -> R + Clone + 'static,
    R: View + 'static,
{
    let children = Rc::new(move |cx: SuspenseContext| children(cx).into_any());

    // 创建属于此 Suspense 边界的上下文
    let ctx = SuspenseContext::new();

    // 在组件初始化时（稳定作用域）执行一次工厂闭包。
    // 确保 Resource 实例绑定到稳定的组件作用域。
    let initial_view = {
        ctx.reset_index();
        children(ctx)
    };

    render! {
        use scope;

        match mode {
            SuspenseMode::KeepAlive => {
                let count = ctx.count;
                view_chain!(
                    div(initial_view.clone())
                        .class("suspense-content")
                        .style(silex_core::rx! {
                            if count.get() > 0 { "display: none" } else { "display: block" }
                    }),
                    div(fallback.clone())
                        .class("suspense-fallback")
                        .style(silex_core::rx! {
                            if count.get() > 0 { "display: block" } else { "display: none" }
                    })
                )
                .into_any()
            }
            SuspenseMode::Unmount => {
                let count = ctx.count;
                let (is_first, set_is_first) = Signal::pair(true);
                let initial_view = initial_view.clone();
                let children = children.clone();
                let fallback = fallback.clone();

                view_chain!(
                    silex_core::rx! {
                        if count.get() == 0 {
                            if is_first.get() {
                                set_is_first.set(false);
                                initial_view.clone()
                            } else {
                                let children = children.clone();
                                ctx.reset_index();
                                render! {
                                    children(ctx)
                                }.into_any()
                            }
                        } else {
                            AnyView::Empty
                        }
                    },
                    silex_core::rx! {
                        if count.get() > 0 {
                            fallback.clone()
                        } else {
                            AnyView::Empty
                        }
                    }
                )
                .into_any()
            }
        }
    }
}
