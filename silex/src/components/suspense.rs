use silex_core::reactivity::{Effect, SuspenseContext, create_scope, use_suspense_context};
use silex_core::traits::Get;
use silex_dom::attribute::GlobalAttributes;
use silex_dom::view::View;
use silex_html::div;
use web_sys::Node;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SuspenseMode {
    #[default]
    KeepAlive,
    Unmount,
}

#[derive(Clone)]
pub struct SuspenseBoundary<C, F> {
    children: C,
    fallback: F,
    mode: SuspenseMode,
    ctx: SuspenseContext,
}

impl SuspenseBoundary<(), ()> {
    pub fn new() -> Self {
        let ctx = use_suspense_context().expect(
            "SuspenseContext not found. Ensure SuspenseBoundary is created inside SuspenseContext::provide closure.",
        );
        Self {
            children: (),
            fallback: (),
            mode: SuspenseMode::default(),
            ctx,
        }
    }
}

impl<C, F> SuspenseBoundary<C, F> {
    pub fn children<NewC>(self, children: NewC) -> SuspenseBoundary<NewC, F> {
        SuspenseBoundary {
            children,
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
            fallback,
            mode: self.mode,
            ctx: self.ctx,
        }
    }
}

impl<C, F, VRes, FRes> View for SuspenseBoundary<C, F>
where
    C: Fn() -> VRes + 'static,
    VRes: View + 'static,
    F: Fn() -> FRes + 'static,
    FRes: View + 'static,
{
    fn mount(self, parent: &Node) {
        let children_fn = std::rc::Rc::new(self.children);
        let fallback_fn = std::rc::Rc::new(self.fallback);
        let mode = self.mode;
        let count = self.ctx.count;

        let parent_clone = parent.clone();

        // create_scope is used to manage cleanups for the boundary logic
        create_scope(move || {
            match mode {
                SuspenseMode::KeepAlive => {
                    let children_fn = children_fn.clone();
                    let fallback_fn = fallback_fn.clone();

                    // 1. Content Wrapper (Hidden when loading)
                    let content_wrapper = div(()).class("suspense-content");
                    let _ = content_wrapper.clone().style(move || {
                        if count.get() > 0 {
                            "display: none"
                        } else {
                            "display: block"
                        }
                    });
                    content_wrapper.clone().mount(&parent_clone);
                    let content_root = content_wrapper.element;

                    Effect::new(move |_| {
                        let view = children_fn();
                        content_root.set_inner_html("");
                        view.mount(&content_root);
                    });

                    // 2. Fallback Wrapper (Visible when loading)
                    let fallback_wrapper = div(()).class("suspense-fallback");
                    let _ = fallback_wrapper.clone().style(move || {
                        if count.get() > 0 {
                            "display: block"
                        } else {
                            "display: none"
                        }
                    });
                    fallback_wrapper.clone().mount(&parent_clone);
                    let fallback_root = fallback_wrapper.element;

                    Effect::new(move |_| {
                        let view = fallback_fn();
                        fallback_root.set_inner_html("");
                        view.mount(&fallback_root);
                    });
                }
                SuspenseMode::Unmount => {
                    let children_fn = children_fn.clone();
                    let fallback_fn = fallback_fn.clone();

                    // 1. Content Wrapper
                    let content_wrapper = div(()).class("suspense-content");
                    content_wrapper.clone().mount(&parent_clone);
                    let content_root = content_wrapper.element;

                    Effect::new(move |_| {
                        if count.get() > 0 {
                            // Suspended: Unmount content
                            content_root.set_inner_html("");
                        } else {
                            // Active: Mount content
                            // Re-executing children_fn re-establishes fine-grained dependencies
                            let view = children_fn();
                            content_root.set_inner_html("");
                            view.mount(&content_root);
                        }
                    });

                    // 2. Fallback Wrapper
                    let fallback_wrapper = div(()).class("suspense-fallback");
                    fallback_wrapper.clone().mount(&parent_clone);
                    let fallback_root = fallback_wrapper.element;

                    Effect::new(move |_| {
                        if count.get() > 0 {
                            // Suspended: Show Fallback
                            let view = fallback_fn();
                            fallback_root.set_inner_html("");
                            view.mount(&fallback_root);
                        } else {
                            // Active: Unmount Fallback
                            fallback_root.set_inner_html("");
                        }
                    });
                }
            }
        });
    }
}
