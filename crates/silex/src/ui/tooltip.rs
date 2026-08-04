use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, tw};
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipContext {
    pub open: RwSignal<bool>,
    pub anchor: RwSignal<(f64, f64, f64, f64)>,
    pub timer_id: StoredValue<i32>,
}

impl TooltipContext {
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            anchor: RwSignal::new((0.0, 0.0, 0.0, 0.0)),
            timer_id: StoredValue::new(0i32),
        }
    }

    /// Cancels any pending close timeout.
    pub fn cancel_close_timer(&self) {
        let handle = self.timer_id.get_untracked();
        if handle != 0 {
            if let Some(w) = web_sys::window() {
                w.clear_timeout_with_handle(handle);
            }
            self.timer_id.set(0);
        }
    }

    /// Schedules a close timeout after `delay_ms` milliseconds (default 150ms grace period).
    pub fn schedule_close_timer(&self, delay_ms: i32) {
        self.cancel_close_timer();
        let open_sig = self.open;
        let timer_id = self.timer_id;
        let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
            timer_id.set(0);
            open_sig.set(false);
        });
        if let Some(w) = web_sys::window()
            && let Ok(handle) = w.set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                delay_ms,
            )
        {
            self.timer_id.set(handle);
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

impl Default for TooltipContext {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn TooltipProvider(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    delay_duration: Signal<f64>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let provider_cls = rx!(move || {
        let base = tw!("relative");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let delay_attr = rx!(move || {
        let d = delay_duration.get();
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
pub fn Tooltip<C, V>(
    children: C,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View
where
    C: Fn(TooltipContext) -> V + 'static,
    V: View + 'static,
{
    let ctx = TooltipContext::new();
    let root_cls = rx!(move || {
        let base = tw!("relative inline-block");
        let extra = class.get();
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
pub fn TooltipTrigger(
    children: AnyView,
    #[chain] ctx: TooltipContext,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_mouse_enter: Callback<web_sys::MouseEvent>,
    #[prop(into)]
    #[chain(default)]
    on_mouse_leave: Callback<web_sys::MouseEvent>,
    #[prop(into)]
    #[chain(default)]
    on_focus: Callback<web_sys::FocusEvent>,
    #[prop(into)]
    #[chain(default)]
    on_blur: Callback<web_sys::FocusEvent>,
) -> impl View {
    let trigger_cls = rx!(move || {
        let base = tw!("inline-block cursor-pointer");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children)
        .attr("data-slot", "tooltip-trigger")
        .class(trigger_cls)
        .on(event::mouseenter, move |e: web_sys::MouseEvent| {
            ctx.anchor.set(get_event_anchor(&e));
            ctx.on_pointer_enter();
            let _ = on_mouse_enter.invoke(e);
        })
        .on(event::mouseleave, move |e: web_sys::MouseEvent| {
            ctx.on_pointer_leave();
            let _ = on_mouse_leave.invoke(e);
        })
        .on(event::focus, move |e: web_sys::FocusEvent| {
            let target = e.current_target().or_else(|| e.target());
            if let Some(target) = target
                && let Ok(el) = target.dyn_into::<web_sys::Element>()
            {
                ctx.anchor.set(get_element_anchor(&el));
            }
            ctx.on_pointer_enter();
            let _ = on_focus.invoke(e);
        })
        .on(event::blur, move |e: web_sys::FocusEvent| {
            ctx.on_pointer_leave();
            let _ = on_blur.invoke(e);
        })
}

#[component]
pub fn TooltipContent(
    children: AnyView,
    #[chain] ctx: TooltipContext,
    #[prop(into)]
    #[chain(default)]
    side: Signal<String>, // top | bottom | left | right
    #[prop(into)]
    #[chain(default)]
    side_offset: Signal<f64>, // offset from trigger element in px
    #[prop(into)]
    #[chain(default)]
    hide_arrow: Signal<bool>, // set true to hide the arrow
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let stored_children = StoredValue::new(children);

    let side_val = rx!(move || {
        let s = side.get();
        if s.is_empty() { "top".to_string() } else { s }
    });

    let offset_val = rx!(move || {
        let o = side_offset.get();
        if o == 0.0 { 4.0 } else { o }
    });

    let wrapper_cls =
        tw!("fixed left-0 top-0 z-50 min-w-max will-change-transform pointer-events-none");

    let wrapper_style = rx!(move || {
        let (t, l, w, h) = ctx.anchor.get();
        let s = side_val.get();
        let offset = offset_val.get();

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

    let content_cls = rx!(move || {
        let s = side_val.get();
        let pos_cls = match s.as_str() {
            "bottom" => tw!("slide-in-from-top-2 -translate-x-1/2"),
            "left" => tw!("slide-in-from-right-2 -translate-x-full -translate-y-1/2"),
            "right" => tw!("slide-in-from-left-2 -translate-y-1/2"),
            _ => tw!("slide-in-from-bottom-2 -translate-x-1/2 -translate-y-full"),
        };

        let base = tw!(
            "relative z-50 w-fit rounded-md bg-slate-900 px-3 py-1.5 text-xs text-slate-50 shadow-md animate-in fade-in-0 zoom-in-95 whitespace-nowrap dark:bg-slate-50 dark:text-slate-900 pointer-events-auto"
        );
        let extra = class.get();
        if extra.is_empty() {
            format!("{} {}", base, pos_cls)
        } else {
            format!("{} {} {}", base, pos_cls, extra)
        }
    });

    let arrow_cls = rx!(move || {
        let s = side_val.get();
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

    rx!(move || {
        if ctx.open.get() {
            let children_view = stored_children.get();
            let arrow_view = if !hide_arrow.get() {
                div(()).class(arrow_cls).into_any()
            } else {
                ().into_any()
            };

            crate::components::Portal(
                div(div(view_chain!(children_view, arrow_view))
                    .attr("data-slot", "tooltip-content")
                    .attr("data-side", rx!(move || side_val.get()))
                    .attr("data-align", "center")
                    .attr("data-state", "delayed-open")
                    .attr("role", "tooltip")
                    .class(content_cls)
                    .on(event::mouseenter, move |_| {
                        ctx.on_pointer_enter();
                    })
                    .on(event::mouseleave, move |_| {
                        ctx.on_pointer_leave();
                    }))
                .attr("data-radix-popper-content-wrapper", "")
                .class(wrapper_cls)
                .attr("style", wrapper_style),
            )
            .into_any()
        } else {
            ().into_any()
        }
    })
}
