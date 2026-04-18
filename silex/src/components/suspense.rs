use silex_core::reactivity::{Effect, SuspenseContext, create_scope, use_suspense_context};
use silex_core::traits::RxGet;
use silex_dom::attribute::GlobalAttributes;
use silex_dom::view::View;
use silex_html::div;
use std::rc::Rc;
use web_sys::Node;

/// A builder for creating a Suspense context and providing resources.
///
/// # Example
/// ```rust,ignore
/// Suspense::new()
///     .resource(|| Resource::new(source, fetcher))
///     .children(|resource| {
///         SuspenseBoundary::new()
///             .fallback(|| "Loading...")
///             .children(move || resource.get())
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

    pub fn resource<R, F>(self, f: F) -> Suspense<F>
    where
        F: FnOnce() -> R,
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

pub struct SuspenseBoundary<C, F> {
    children: Rc<C>,
    fallback: Rc<F>,
    mode: SuspenseMode,
    ctx: SuspenseContext,
}

impl<C, F> Clone for SuspenseBoundary<C, F> {
    fn clone(&self) -> Self {
        Self {
            children: self.children.clone(),
            fallback: self.fallback.clone(),
            mode: self.mode,
            ctx: self.ctx,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SuspenseMode {
    #[default]
    KeepAlive,
    Unmount,
}

impl Default for SuspenseBoundary<(), ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl SuspenseBoundary<(), ()> {
    pub fn new() -> Self {
        let ctx = use_suspense_context().expect(
            "SuspenseContext not found. Ensure SuspenseBoundary is created inside SuspenseContext::provide closure.",
        );
        Self {
            children: Rc::new(()),
            fallback: Rc::new(()),
            mode: SuspenseMode::default(),
            ctx,
        }
    }
}

impl<C, F> SuspenseBoundary<C, F> {
    pub fn children<NewC>(self, children: NewC) -> SuspenseBoundary<NewC, F> {
        SuspenseBoundary {
            children: Rc::new(children),
            fallback: self.fallback,
            mode: self.mode,
            ctx: self.ctx,
        }
    }

    pub fn mode(mut self, mode: SuspenseMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn fallback<NewF>(self, fallback: NewF) -> SuspenseBoundary<C, NewF> {
        SuspenseBoundary {
            children: self.children,
            fallback: Rc::new(fallback),
            mode: self.mode,
            ctx: self.ctx,
        }
    }
}

impl<C, F, VRes, FRes> View for SuspenseBoundary<C, F>
where
    C: crate::flow::ViewFactory<View = VRes> + 'static,
    VRes: View + 'static,
    F: crate::flow::ViewFactory<View = FRes> + 'static,
    FRes: View + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        let children_fn = self.children;
        let fallback_fn = self.fallback;
        let mode = self.mode;
        let count = self.ctx.count;

        let parent_clone = parent.clone();

        create_scope(move || match mode {
            SuspenseMode::KeepAlive => {
                let children_fn = children_fn.clone();
                let fallback_fn = fallback_fn.clone();

                let content_wrapper = div(()).class("suspense-content");
                let _ = content_wrapper.clone().style(silex_core::rx! {
                    if count.get() > 0 { "display: none" } else { "display: block" }
                });
                content_wrapper.clone().mount(&parent_clone, attrs.clone());
                let content_root = content_wrapper.element;

                Effect::new(move |_| {
                    let view = children_fn.render();
                    content_root.set_inner_html("");
                    view.mount(&content_root, Vec::new());
                });

                let fallback_wrapper = div(()).class("suspense-fallback");
                let _ = fallback_wrapper.clone().style(silex_core::rx! {
                    if count.get() > 0 { "display: block" } else { "display: none" }
                });
                fallback_wrapper.clone().mount(&parent_clone, Vec::new());
                let fallback_root = fallback_wrapper.element;

                Effect::new(move |_| {
                    let view = fallback_fn.render();
                    fallback_root.set_inner_html("");
                    view.mount(&fallback_root, Vec::new());
                });
            }
            SuspenseMode::Unmount => {
                let children_fn = children_fn.clone();
                let fallback_fn = fallback_fn.clone();

                let content_wrapper = div(()).class("suspense-content");
                content_wrapper.clone().mount(&parent_clone, attrs);
                let content_root = content_wrapper.element;

                Effect::new(move |_| {
                    if count.get() > 0 {
                        content_root.set_inner_html("");
                    } else {
                        let view = children_fn.render();
                        content_root.set_inner_html("");
                        view.mount(&content_root, Vec::new());
                    }
                });

                let fallback_wrapper = div(()).class("suspense-fallback");
                fallback_wrapper.clone().mount(&parent_clone, Vec::new());
                let fallback_root = fallback_wrapper.element;

                Effect::new(move |_| {
                    if count.get() > 0 {
                        let view = fallback_fn.render();
                        fallback_root.set_inner_html("");
                        view.mount(&fallback_root, Vec::new());
                    } else {
                        fallback_root.set_inner_html("");
                    }
                });
            }
        });
    }

    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        self.clone().mount(parent, attrs);
    }
}
