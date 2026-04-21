use silex_core::reactivity::{SuspenseContext, create_scope, use_suspense_context};
use silex_core::traits::RxGet;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::component;
use web_sys::Node;

/// A builder for creating a Suspense context and providing resources.
///
/// # Example
/// ```rust,ignore
/// Suspense::new()
///     .resource(|| Resource::new(source, fetcher))
///     .children(|resource| {
///         SuspenseBoundary(move || resource.get(), || "Loading...")
///     })
/// ```
pub struct Suspense<F = ()> {
    resource_factory: F,
}

impl Default for Suspense<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl Suspense<()> {
    pub fn new() -> Self {
        Self {
            resource_factory: (),
        }
    }

    pub fn resource<R, FB>(self, f: FB) -> Suspense<FB>
    where
        FB: FnOnce() -> R,
    {
        Suspense {
            resource_factory: f,
        }
    }
}

impl<F, R> Suspense<F>
where
    F: FnOnce() -> R,
{
    pub fn children<V, C>(self, child_fn: C) -> V
    where
        C: FnOnce(R) -> V,
    {
        SuspenseContext::provide(|| {
            let resource = (self.resource_factory)();
            child_fn(resource)
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SuspenseMode {
    #[default]
    KeepAlive,
    Unmount,
}

/// SuspenseBoundary 组件
///
/// 用于处理异步加载状态，根据 SuspenseContext 的状态切换显示 children 或 fallback。
#[component]
pub fn SuspenseBoundary<CH, FB>(
    #[prop(clone)] children: CH,
    #[prop(clone)] fallback: FB,
    #[prop(default)] mode: SuspenseMode,
    #[prop(default = use_suspense_context().expect("SuspenseContext not found. Ensure SuspenseBoundary is created inside SuspenseContext::provide closure."))]
    ctx: SuspenseContext,
) -> impl Mount + MountRef
where
    CH: MountExt + Clone + 'static,
    FB: MountExt + Clone + 'static,
{
    // 从 Prop 中提取原始值，确保 View 是 'static 的
    // 由于 ctx 具有默认值且不是第一个参数，它由宏处理为 Prop<SuspenseContext>
    SuspenseBoundaryView {
        children: children.clone(),
        fallback: fallback.clone(),
        mode: *mode,
        ctx: *ctx,
    }
}

struct SuspenseBoundaryView<CH, FB> {
    children: CH,
    fallback: FB,
    mode: SuspenseMode,
    ctx: SuspenseContext,
}

impl<CH, FB> ApplyAttributes for SuspenseBoundaryView<CH, FB> {}

impl<CH, FB> Mount for SuspenseBoundaryView<CH, FB>
where
    CH: MountExt + Clone + 'static,
    FB: MountExt + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let children_fn = self.children;
        let fallback_fn = self.fallback;
        let mode = self.mode;
        let count = self.ctx.count;
        let parent_clone = parent.clone();

        create_scope(move || match mode {
            SuspenseMode::KeepAlive => {
                let children_fn = children_fn.clone();
                let fallback_fn = fallback_fn.clone();

                // Content wrapper (KeepAlive: always in DOM, toggle visibility)
                let content_wrapper = div(()).class("suspense-content");
                let count_clone = count;
                let _ = content_wrapper.clone().style(silex_core::rx! {
                    if count_clone.get() > 0 { "display: none" } else { "display: block" }
                });
                content_wrapper.clone().mount(&parent_clone, attrs);
                children_fn.mount_ref(&content_wrapper.element, Vec::new());

                // Fallback wrapper (KeepAlive: always in DOM, toggle visibility)
                let fallback_wrapper = div(()).class("suspense-fallback");
                let count_clone = count;
                let _ = fallback_wrapper.clone().style(silex_core::rx! {
                    if count_clone.get() > 0 { "display: block" } else { "display: none" }
                });
                fallback_wrapper.clone().mount(&parent_clone, Vec::new());
                fallback_fn.mount_ref(&fallback_wrapper.element, Vec::new());
            }
            SuspenseMode::Unmount => {
                let children_fn = children_fn.clone();
                let fallback_fn = fallback_fn.clone();

                // Dynamic mounting for children
                let count_clone = count;
                let children_rx = silex_core::rx! {
                    if count_clone.get() == 0 {
                        children_fn.clone().into_any()
                    } else {
                        ().into_any()
                    }
                };
                children_rx.mount(&parent_clone, attrs);

                // Dynamic mounting for fallback
                let count_clone = count;
                let fallback_rx = silex_core::rx! {
                    if count_clone.get() > 0 {
                        fallback_fn.clone().into_any()
                    } else {
                        ().into_any()
                    }
                };
                fallback_rx.mount(&parent_clone, Vec::new());
            }
        });
    }
}

impl<CH, FB> AutoReactiveView for SuspenseBoundaryView<CH, FB>
where
    CH: MountExt + Clone + 'static,
    FB: MountExt + Clone + 'static,
{
}

impl<CH, FB> MountRef for SuspenseBoundaryView<CH, FB>
where
    CH: MountExt + Clone + 'static,
    FB: MountExt + Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let view = SuspenseBoundaryView {
            children: self.children.clone(),
            fallback: self.fallback.clone(),
            mode: self.mode,
            ctx: self.ctx,
        };
        view.mount(parent, attrs);
    }
}
