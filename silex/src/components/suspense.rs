use silex_core::reactivity::{SuspenseContext, create_scope};
use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::component;
use std::marker::PhantomData;
use web_sys::Node;

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
    #[prop(default = AnyView::Empty)] fallback: AnyView,
    #[prop(default)] mode: SuspenseMode,
) -> impl Mount + MountRef
where
    CH: Fn() -> R + Clone + 'static,
    R: MountExt,
{
    // 创建属于此 Suspense 边界的上下文
    let ctx = SuspenseContext::new();

    // 关键点：在组件初始化时（稳定作用域）执行一次工厂闭包。
    // 这有三个目的：
    // 1. 发现并创建所有 Resource 实例，并将它们缓存在 SuspenseContext 中。
    // 2. 确保 Resource 实例绑定到稳定的组件作用域，而不是 Unmount 模式下临时的挂载作用域。
    // 3. 生成初始视图供第一次挂载或 KeepAlive 模式使用。
    let initial_view = SuspenseContext::provide_with(ctx.clone(), {
        let children = children.clone();
        move || children().into_any()
    });

    SuspenseView {
        factory: children.clone(),
        initial_view,
        fallback: fallback.clone(),
        mode: *mode,
        ctx,
        _marker: PhantomData,
    }
}

struct SuspenseView<CH, R> {
    factory: CH,
    initial_view: AnyView,
    fallback: AnyView,
    mode: SuspenseMode,
    ctx: SuspenseContext,
    _marker: PhantomData<R>,
}

impl<CH, R> ApplyAttributes for SuspenseView<CH, R> {}

impl<CH, R> Mount for SuspenseView<CH, R>
where
    CH: Fn() -> R + Clone + 'static,
    R: MountExt,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let factory = self.factory;
        let initial_view = self.initial_view;
        let fallback_view = self.fallback;
        let mode = self.mode;
        let parent_clone = parent.clone();
        let count = self.ctx.count;
        let ctx = self.ctx;

        create_scope(move || match mode {
            SuspenseMode::KeepAlive => {
                let children_view = initial_view;
                let fallback_view = fallback_view.clone();

                // KeepAlive 模式：始终保留 DOM 节点，仅切换显示状态
                let content_wrapper = div(()).class("suspense-content");
                let count_clone = count;
                let _ = content_wrapper.clone().style(silex_core::rx! {
                    if count_clone.get() > 0 { "display: none" } else { "display: block" }
                });
                content_wrapper.clone().mount(&parent_clone, attrs);
                children_view.mount_ref(&content_wrapper.element, Vec::new());

                let fallback_wrapper = div(()).class("suspense-fallback");
                let count_clone = count;
                let _ = fallback_wrapper.clone().style(silex_core::rx! {
                    if count_clone.get() > 0 { "display: block" } else { "display: none" }
                });
                fallback_wrapper.clone().mount(&parent_clone, Vec::new());
                fallback_view.mount_ref(&fallback_wrapper.element, Vec::new());
            }
            SuspenseMode::Unmount => {
                let factory = factory.clone();
                let initial_view = initial_view;
                let fallback_view = fallback_view.clone();
                let ctx_clone = ctx.clone();

                // 用于标记是否是第一次渲染
                let is_first = std::rc::Rc::new(std::cell::Cell::new(true));

                // Unmount 模式：真正从 DOM 中移除/添加节点
                let count_clone = count;
                let children_rx = silex_core::rx! {
                    if count_clone.get() == 0 {
                        if is_first.get() {
                            is_first.set(false);
                            initial_view.clone()
                        } else {
                            // 重新执行工厂闭包以生成全新的视图（重置本地 DOM 状态）
                            // 内部的 Resource::new 会从缓存中获取稳定的 Resource 实例
                            SuspenseContext::provide_with(ctx_clone.clone(), factory.clone()).into_any()
                        }
                    } else {
                        ().into_any()
                    }
                };
                children_rx.mount(&parent_clone, attrs);

                let count_clone = count;
                let fallback_rx = silex_core::rx! {
                    if count_clone.get() > 0 {
                        fallback_view.clone()
                    } else {
                        ().into_any()
                    }
                };
                fallback_rx.mount(&parent_clone, Vec::new());
            }
        });
    }
}

impl<CH, R> AutoReactiveView for SuspenseView<CH, R>
where
    CH: Fn() -> R + Clone + 'static,
    R: MountExt,
{
}

impl<CH, R> MountRef for SuspenseView<CH, R>
where
    CH: Fn() -> R + Clone + 'static,
    R: MountExt,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let view = SuspenseView {
            factory: self.factory.clone(),
            initial_view: self.initial_view.clone(),
            fallback: self.fallback.clone(),
            mode: self.mode,
            ctx: self.ctx.clone(),
            _marker: PhantomData,
        };
        view.mount(parent, attrs);
    }
}
