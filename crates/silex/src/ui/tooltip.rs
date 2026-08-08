use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_dom::view::ViewOwnerToken;
use silex_html::div;
use silex_macros::{component, tw};
use std::time::Duration;
use wasm_bindgen::JsCast;

/// Helper to extract bounding rectangle (top, left, width, height) from a MouseEvent target.
fn get_event_anchor(e: &web_sys::MouseEvent) -> (f64, f64, f64, f64) {
    let target = e.current_target().or_else(|| e.target());
    if let Some(target) = target
        && let Ok(el) = target.dyn_into::<web_sys::Element>()
    {
        get_element_anchor(&el)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}

/// Helper to extract bounding rectangle (top, left, width, height) from an Element.
fn get_element_anchor(el: &web_sys::Element) -> (f64, f64, f64, f64) {
    let rect = el.get_bounding_client_rect();
    (rect.top(), rect.left(), rect.width(), rect.height())
}

/// Explicit Tooltip Context holding reactive state for visibility, anchor positioning, and hover timers.
#[derive(Clone, Copy)]
pub struct TooltipContext<'scope> {
    pub open: RwSignal<'scope, bool>,
    pub anchor: RwSignal<'scope, (f64, f64, f64, f64)>,
    timer: StoredValue<'scope, Option<HostResourceHandle<'scope>>>,
    owner: StoredValue<'scope, Option<ViewOwnerToken<'scope>>>,
}

impl<'scope> TooltipContext<'scope> {
    pub fn new(scope: Scope<'scope>) -> Self {
        Self {
            open: scope.rw_signal(false),
            anchor: scope.rw_signal((0.0, 0.0, 0.0, 0.0)),
            timer: scope.stored(None),
            owner: scope.stored(None),
        }
    }

    fn set_owner(&self, owner: ViewOwnerToken<'scope>) {
        self.owner.set(Some(owner));
    }

    /// Cancels any pending close timeout.
    pub fn cancel_close_timer(&self) {
        let _ = self.timer.try_update(|timer| {
            if let Some(handle) = timer.take() {
                handle.cancel();
            }
        });
    }

    /// Schedules a close timeout after `delay_ms` milliseconds (default 150ms grace period).
    pub fn schedule_close_timer(&self, delay_ms: i32) {
        self.cancel_close_timer();
        let Some(owner) = self.owner.try_with(Clone::clone).ok().flatten() else {
            return;
        };
        let open = self.open;
        let timer = self.timer;
        if let Ok(handle) = set_timeout_owned(
            &owner,
            move || -> SilexResult<()> {
                timer.try_update(|slot| *slot = None)?;
                open.set(false);
                Ok(())
            },
            Duration::from_millis(delay_ms.max(0) as u64),
        ) {
            self.timer.set(Some(handle));
        }
    }

    /// Called on pointer enter (trigger or content): cancels closing and opens tooltip.
    pub fn on_pointer_enter(&self) {
        self.cancel_close_timer();
        self.open.set(true);
    }

    /// Called on pointer leave (trigger or content): starts 150ms grace period before closing.
    pub fn on_pointer_leave(&self) {
        self.schedule_close_timer(150);
    }

    pub fn open(&self) {
        self.on_pointer_enter();
    }

    pub fn close(&self) {
        self.cancel_close_timer();
        self.open.set(false);
    }

    pub fn toggle(&self) {
        self.cancel_close_timer();
        self.open.update(|v| *v = !*v);
    }
}

fn owner_binding<'scope>(ctx: TooltipContext<'scope>) -> AttrOp<'scope> {
    AttrOp::custom_with_inputs(RuntimeInputs::new(), move |_, owner| {
        ctx.set_owner(owner.clone());
        Ok(())
    })
}

#[component]
pub fn TooltipProvider<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    delay_duration: Signal<'scope, f64>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let provider_cls = rx!(scope; {
        let base = tw!("relative");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let delay_attr = rx!(scope; {
        let d = *$delay_duration;
        if d > 0.0 {
            format!("{:.0}ms", d)
        } else {
            "0ms".to_string()
        }
    });

    div(children)
        .attr("data-slot", "tooltip-provider")
        .attr("data-delay-duration", delay_attr)
        .class(provider_cls)
}

#[component]
pub fn Tooltip<'scope, C, V>(
    scope: Scope<'scope>,
    children: C,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope>
where
    C: Fn(TooltipContext<'scope>) -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    let ctx = TooltipContext::new(scope);
    let root_cls = rx!(scope; {
        let base = tw!("relative inline-block");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children(ctx))
        .attr("data-slot", "tooltip")
        .class(root_cls)
}

#[component]
pub fn TooltipTrigger<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] ctx: TooltipContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_mouse_enter: Callback<'scope, web_sys::MouseEvent>,
    #[prop(into)]
    #[chain(default)]
    on_mouse_leave: Callback<'scope, web_sys::MouseEvent>,
    #[prop(into)]
    #[chain(default)]
    on_focus: Callback<'scope, web_sys::FocusEvent>,
    #[prop(into)]
    #[chain(default)]
    on_blur: Callback<'scope, web_sys::FocusEvent>,
) -> impl View<'scope> {
    let trigger_cls = rx!(scope; {
        let base = tw!("inline-block cursor-pointer");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children)
        .apply(owner_binding(ctx))
        .attr("data-slot", "tooltip-trigger")
        .class(trigger_cls)
        .on(
            event::mouseenter,
            move |e: web_sys::MouseEvent| -> SilexResult<()> {
                ctx.anchor.set(get_event_anchor(&e));
                ctx.on_pointer_enter();
                on_mouse_enter.invoke(e)
            },
        )
        .on(
            event::mouseleave,
            move |e: web_sys::MouseEvent| -> SilexResult<()> {
                ctx.on_pointer_leave();
                on_mouse_leave.invoke(e)
            },
        )
        .on(
            event::focus,
            move |e: web_sys::FocusEvent| -> SilexResult<()> {
                let target = e.current_target().or_else(|| e.target());
                if let Some(target) = target
                    && let Ok(el) = target.dyn_into::<web_sys::Element>()
                {
                    ctx.anchor.set(get_element_anchor(&el));
                }
                ctx.on_pointer_enter();
                on_focus.invoke(e)
            },
        )
        .on(
            event::blur,
            move |e: web_sys::FocusEvent| -> SilexResult<()> {
                ctx.on_pointer_leave();
                on_blur.invoke(e)
            },
        )
}

#[component]
pub fn TooltipContent<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] ctx: TooltipContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    side: Signal<'scope, String>, // top | bottom | left | right
    #[prop(into)]
    #[chain(default)]
    side_offset: Signal<'scope, f64>, // offset from trigger element in px
    #[prop(into)]
    #[chain(default)]
    hide_arrow: Signal<'scope, bool>, // set true to hide the arrow
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let stored_children = scope.stored(children);

    let side_val = rx!(scope; {
        let s = (*$side).clone();
        if s.is_empty() { "top".to_string() } else { s }
    });

    let offset_val = rx!(scope; {
        let o = *$side_offset;
        if o == 0.0 { 4.0 } else { o }
    });

    let wrapper_cls =
        tw!("fixed left-0 top-0 z-50 min-w-max will-change-transform pointer-events-none");

    let anchor = ctx.anchor;
    let wrapper_style = rx!(scope; {
        let (t, l, w, h) = *$anchor;
        let s = (*$side_val).clone();
        let offset = *$offset_val;

        let (x, y) = match s.as_str() {
            "bottom" => (l + w / 2.0, t + h + offset),
            "left" => (l - offset, t + h / 2.0),
            "right" => (l + w + offset, t + h / 2.0),
            _ => (l + w / 2.0, t - offset), // default top
        };
        format!(
            "position: fixed; left: 0px; top: 0px; transform: translate({:.2}px, {:.2}px);",
            x, y
        )
    });

    let content_cls = rx!(scope; {
        let s = (*$side_val).clone();
        let pos_cls = match s.as_str() {
            "bottom" => tw!("slide-in-from-top-2 -translate-x-1/2"),
            "left" => tw!("slide-in-from-right-2 -translate-x-full -translate-y-1/2"),
            "right" => tw!("slide-in-from-left-2 -translate-y-1/2"),
            _ => tw!("slide-in-from-bottom-2 -translate-x-1/2 -translate-y-full"),
        };

        let base = tw!(
            "relative z-50 w-fit rounded-md bg-slate-900 px-3 py-1.5 text-xs text-slate-50 shadow-md animate-in fade-in-0 zoom-in-95 whitespace-nowrap dark:bg-slate-50 dark:text-slate-900 pointer-events-auto"
        );
        let extra = $class;
        if extra.is_empty() {
            format!("{} {}", base, pos_cls)
        } else {
            format!("{} {} {}", base, pos_cls, extra)
        }
    });

    let arrow_cls = rx!(scope; {
        let s = (*$side_val).clone();
        match s.as_str() {
            "bottom" => tw!(
                "absolute top-0 left-1/2 -translate-x-1/2 -translate-y-1/2 size-2 rotate-45 bg-slate-900 dark:bg-slate-50 pointer-events-none"
            ),
            "left" => tw!(
                "absolute right-0 top-1/2 translate-x-1/2 -translate-y-1/2 size-2 rotate-45 bg-slate-900 dark:bg-slate-50 pointer-events-none"
            ),
            "right" => tw!(
                "absolute left-0 top-1/2 -translate-x-1/2 -translate-y-1/2 size-2 rotate-45 bg-slate-900 dark:bg-slate-50 pointer-events-none"
            ),
            _ => tw!(
                "absolute bottom-0 left-1/2 -translate-x-1/2 translate-y-1/2 size-2 rotate-45 bg-slate-900 dark:bg-slate-50 pointer-events-none"
            ),
        }
    });

    let ctx_open = ctx.open;
    rx!(scope; {
        if *$ctx_open {
            let children_view = stored_children.with(|children| children.clone());
            let arrow_view = if !*$hide_arrow {
                div(()).class(arrow_cls).into_any()
            } else {
                ().into_any()
            };

            crate::components::Portal(
                div(div(chain!(children_view, arrow_view))
                    .attr("data-slot", "tooltip-content")
                    .attr("data-side", side_val)
                    .attr("data-align", "center")
                    .attr("data-state", "delayed-open")
                    .attr("role", "tooltip")
                    .class(content_cls)
                    .on(event::mouseenter, move |_| -> SilexResult<()> {
                        ctx.on_pointer_enter();
                        Ok(())
                    })
                    .on(event::mouseleave, move |_| -> SilexResult<()> {
                        ctx.on_pointer_leave();
                        Ok(())
                    }))
                .attr("data-radix-popper-content-wrapper", "")
                .class(wrapper_cls)
                .attr("style", wrapper_style)
                .apply(owner_binding(ctx)),
            )
            .into_any()
        } else {
            ().into_any()
        }
    })
}
