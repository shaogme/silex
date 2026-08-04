use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{div, p};
use silex_macros::{component, tw};
use wasm_bindgen::JsCast;

/// Explicit Popover Context holding reactive state for visibility and anchor bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverContext {
    pub open: RwSignal<bool>,
    pub anchor_rect: RwSignal<(f64, f64, f64, f64)>,
    pub content_height: RwSignal<f64>,
}

impl PopoverContext {
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            anchor_rect: RwSignal::new((0.0, 0.0, 0.0, 0.0)),
            content_height: RwSignal::new(0.0),
        }
    }

    pub fn update_anchor_from_element(&self, el: &web_sys::Element) {
        let rect = el.get_bounding_client_rect();
        self.anchor_rect
            .set((rect.top(), rect.left(), rect.width(), rect.height()));
    }

    pub fn update_content_from_element(&self, el: &web_sys::Element) {
        let rect = el.get_bounding_client_rect();
        if rect.height() > 0.0 {
            self.content_height.set(rect.height());
        }
    }

    pub fn open(&self) {
        self.open.set(true);
    }

    pub fn close(&self) {
        self.open.set(false);
    }

    pub fn toggle(&self) {
        self.open.update(|v| *v = !*v);
    }
}

impl Default for PopoverContext {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn PopoverHeader(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let header_cls = rx!(move || {
        let base = tw!("flex flex-col gap-1 text-sm");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children)
        .attr("data-slot", "popover-header")
        .class(header_cls)
}

#[component]
pub fn PopoverTitle(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let title_cls = rx!(move || {
        let base = tw!("font-medium leading-none");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children)
        .attr("data-slot", "popover-title")
        .class(title_cls)
}

#[component]
pub fn PopoverDescription(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let desc_cls = rx!(move || {
        let base = tw!("text-muted-foreground text-sm m-0");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    p(children)
        .attr("data-slot", "popover-description")
        .class(desc_cls)
}

#[component]
pub fn PopoverPortal(children: AnyView) -> impl View {
    crate::components::Portal(children)
}

#[component]
pub fn PopoverAnchor(
    children: AnyView,
    #[chain] ctx: PopoverContext,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let anchor_cls = rx!(move || {
        let base = tw!("inline-block");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children)
        .attr("data-slot", "popover-anchor")
        .class(anchor_cls)
        .on(event::click, move |e: web_sys::MouseEvent| {
            let target = e.current_target().or_else(|| e.target());
            if let Some(target) = target
                && let Ok(el) = target.dyn_into::<web_sys::Element>()
            {
                ctx.update_anchor_from_element(&el);
            }
        })
}

#[component]
pub fn PopoverClose(
    children: AnyView,
    #[chain] ctx: PopoverContext,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<()>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let close_cls = rx!(move || {
        let base = tw!("inline-flex items-center justify-center cursor-pointer");
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    div(children)
        .attr("data-slot", "popover-close")
        .class(close_cls)
        .on(event::click, move |_| {
            ctx.close();
            let _ = on_click.invoke(());
        })
}

#[component]
pub fn PopoverContent(
    children: AnyView,
    #[chain] ctx: PopoverContext,
    #[prop(into)]
    #[chain(default)]
    open: Signal<bool>,
    #[prop(into)]
    #[chain(default)]
    side: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    align: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    side_offset: Signal<f64>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_close: Callback<()>,
) -> impl View {
    let side_val = rx!(move || {
        let s = side.get();
        if s.is_empty() {
            "bottom".to_string()
        } else {
            s
        }
    });

    let align_val = rx!(move || {
        let a = align.get();
        if a.is_empty() {
            "center".to_string()
        } else {
            a
        }
    });

    let offset_val = rx!(move || {
        let o = side_offset.get();
        if o == 0.0 { 4.0 } else { o }
    });

    let wrapper_cls =
        tw!("fixed left-0 top-0 z-50 min-w-max will-change-transform pointer-events-none");

    let wrapper_style = rx!(move || {
        let (t, l, w, h) = ctx.anchor_rect.get();
        let s = side_val.get();
        let a = align_val.get();
        let offset = offset_val.get();

        let (x, y) = match s.as_str() {
            "top" => {
                let y = t - offset;
                let x = match a.as_str() {
                    "start" => l,
                    "end" => l + w,
                    _ => l + w / 2.0,
                };
                (x, y)
            }
            "left" => {
                let x = l - offset;
                let y = match a.as_str() {
                    "start" => t,
                    "end" => t + h,
                    _ => t + h / 2.0,
                };
                (x, y)
            }
            "right" => {
                let x = l + w + offset;
                let y = match a.as_str() {
                    "start" => t,
                    "end" => t + h,
                    _ => t + h / 2.0,
                };
                (x, y)
            }
            _ => {
                let y = t + h + offset;
                let x = match a.as_str() {
                    "start" => l,
                    "end" => l + w,
                    _ => l + w / 2.0,
                };
                (x, y)
            }
        };

        format!(
            "position: fixed; left: 0px; top: 0px; transform: translate({:.2}px, {:.2}px); --radix-popover-content-transform-origin: center; --radix-popover-trigger-width: {:.2}px; --radix-popover-trigger-height: {:.2}px;",
            x, y, w, h
        )
    });

    let content_cls = rx!(move || {
        let s = side_val.get();
        let a = align_val.get();

        let pos_cls = match s.as_str() {
            "top" => match a.as_str() {
                "start" => tw!("slide-in-from-bottom-2 -translate-y-full"),
                "end" => tw!("slide-in-from-bottom-2 -translate-x-full -translate-y-full"),
                _ => tw!("slide-in-from-bottom-2 -translate-x-1/2 -translate-y-full"),
            },
            "left" => match a.as_str() {
                "start" => tw!("slide-in-from-right-2 -translate-x-full"),
                "end" => tw!("slide-in-from-right-2 -translate-x-full -translate-y-full"),
                _ => tw!("slide-in-from-right-2 -translate-x-full -translate-y-1/2"),
            },
            "right" => match a.as_str() {
                "start" => tw!("slide-in-from-left-2"),
                "end" => tw!("slide-in-from-left-2 -translate-y-full"),
                _ => tw!("slide-in-from-left-2 -translate-y-1/2"),
            },
            _ => match a.as_str() {
                "start" => tw!("slide-in-from-top-2"),
                "end" => tw!("slide-in-from-top-2 -translate-x-full"),
                _ => tw!("slide-in-from-top-2 -translate-x-1/2"),
            },
        };

        let base = tw!(
            "z-50 w-72 rounded-md border bg-popover p-4 text-popover-foreground shadow-md outline-hidden data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 pointer-events-auto"
        );
        let extra = class.get();
        if extra.is_empty() {
            format!("{} {}", pos_cls, base)
        } else {
            format!("{} {} {}", pos_cls, base, extra)
        }
    });

    let stored = StoredValue::new(children);
    let is_open = rx!(move || open.get() || ctx.open.get());

    rx!(move || {
        if is_open.get() {
            crate::components::Portal(view_chain!(
                // Overlay for click-outside
                div(()).class(tw!("fixed inset-0 z-50 bg-transparent")).on(
                    event::click,
                    move |_| {
                        ctx.close();
                        let _ = on_close.invoke(());
                    }
                ),
                // Content wrapper
                div(div(stored.get())
                    .attr("data-slot", "popover-content")
                    .attr("data-state", "open")
                    .attr("data-side", rx!(move || side_val.get()))
                    .attr("data-align", rx!(move || align_val.get()))
                    .attr("role", "dialog")
                    .attr("tabindex", "-1")
                    .class(content_cls))
                .attr("data-radix-popper-content-wrapper", "")
                .class(wrapper_cls)
                .attr("style", wrapper_style)
            ))
            .into_any()
        } else {
            ().into_any()
        }
    })
}

#[component]
pub fn PopoverTrigger(
    children: AnyView,
    #[chain] ctx: PopoverContext,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<()>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
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
        .attr("data-slot", "popover-trigger")
        .class(trigger_cls)
        .on(event::click, move |e: web_sys::MouseEvent| {
            let target = e.current_target().or_else(|| e.target());
            if let Some(target) = target
                && let Ok(el) = target.dyn_into::<web_sys::Element>()
            {
                ctx.update_anchor_from_element(&el);
            }
            ctx.toggle();
            let _ = on_click.invoke(());
        })
}

#[component]
pub fn Popover<C, V>(
    children: C,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View
where
    C: Fn(PopoverContext) -> V + 'static,
    V: View + 'static,
{
    let ctx = PopoverContext::new();
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
        .attr("data-slot", "popover")
        .class(root_cls)
}
