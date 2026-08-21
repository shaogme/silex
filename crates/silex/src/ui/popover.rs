use crate::components::{Portal, PortalHostAttrs};
use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{div, p};
use silex_macros::{component, tw};
use wasm_bindgen::JsCast;

/// Explicit Popover Context holding reactive state for visibility and anchor bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverContext<'scope> {
    pub open: RwSignal<'scope, bool>,
    pub anchor_rect: RwSignal<'scope, (f64, f64, f64, f64)>,
    pub content_height: RwSignal<'scope, f64>,
}

impl<'scope> PopoverContext<'scope> {
    pub fn new(owner: OwnerAccess<'scope>) -> SilexResult<Self> {
        Ok(Self {
            open: owner.rw_signal(false)?,
            anchor_rect: owner.rw_signal((0.0, 0.0, 0.0, 0.0))?,
            content_height: owner.rw_signal(0.0)?,
        })
    }

    pub fn update_anchor_from_element(&self, el: &web_sys::Element) -> SilexResult<()> {
        let rect = el.get_bounding_client_rect();
        self.anchor_rect
            .set((rect.top(), rect.left(), rect.width(), rect.height()))
    }

    pub fn update_content_from_element(&self, el: &web_sys::Element) -> SilexResult<()> {
        let rect = el.get_bounding_client_rect();
        if rect.height() > 0.0 {
            self.content_height.set(rect.height())
        } else {
            Ok(())
        }
    }

    pub fn open(&self) -> SilexResult<()> {
        self.open.set(true)
    }

    pub fn close(&self) -> SilexResult<()> {
        self.open.set(false)
    }

    pub fn toggle(&self) -> SilexResult<()> {
        self.open.update(|v| *v = !*v)
    }
}

#[component]
pub fn PopoverHeader<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let header_cls = rx!(ctx; {
        let base = tw!("flex flex-col gap-1 text-sm");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "popover-header")
        .class(header_cls))
}

#[component]
pub fn PopoverTitle<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let title_cls = rx!(ctx; {
        let base = tw!("font-medium leading-none");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "popover-title")
        .class(title_cls))
}

#[component]
pub fn PopoverDescription<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let desc_cls = rx!(ctx; {
        let base = tw!("text-muted-foreground text-sm m-0");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(p(children)
        .attr("data-slot", "popover-description")
        .class(desc_cls))
}

#[component]
pub fn PopoverAnchor<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain] context: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let anchor_cls = rx!(ctx; {
        let base = tw!("inline-block");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "popover-anchor")
        .class(anchor_cls)
        .on(
            event::click,
            move |e: web_sys::MouseEvent| -> SilexResult<()> {
                let target = e.current_target().or_else(|| e.target());
                if let Some(target) = target
                    && let Ok(el) = target.dyn_into::<web_sys::Element>()
                {
                    context.update_anchor_from_element(&el)?;
                }
                Ok(())
            },
        ))
}

#[component]
pub fn PopoverClose<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain] context: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<'scope, ()>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let close_cls = rx!(ctx; {
        let base = tw!("inline-flex items-center justify-center cursor-pointer");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "popover-close")
        .class(close_cls)
        .on(event::click, move |_| -> SilexResult<()> {
            context.close()?;
            on_click.invoke(())
        }))
}

#[component]
pub fn PopoverContent<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain] context: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    open: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    side: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    align: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    side_offset: Signal<'scope, f64>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_close: Callback<'scope, ()>,
) -> impl View<'scope> {
    let side_val = rx!(ctx; {
        let s = (*$side).clone();
        if s.is_empty() {
            "bottom".to_string()
        } else {
            s
        }
    })?;

    let align_val = rx!(ctx; {
        let a = (*$align).clone();
        if a.is_empty() {
            "center".to_string()
        } else {
            a
        }
    })?;

    let offset_val = rx!(ctx; {
        let o = *$side_offset;
        if o == 0.0 { 4.0 } else { o }
    })?;

    let wrapper_cls =
        tw!("fixed left-0 top-0 z-50 min-w-max will-change-transform pointer-events-none");

    let anchor_rect = context.anchor_rect;
    let wrapper_style = rx!(ctx; {
        let (t, l, w, h) = *$anchor_rect;
        let s = (*$side_val).clone();
        let a = (*$align_val).clone();
        let offset = *$offset_val;

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
    })?;

    let content_cls = rx!(ctx; {
        let s = (*$side_val).clone();
        let a = (*$align_val).clone();

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
        let extra = $class;
        if extra.is_empty() {
            format!("{} {}", pos_cls, base)
        } else {
            format!("{} {} {}", pos_cls, base, extra)
        }
    })?;

    let stored = owner.stored(children)?;
    let ctx_open = context.open;
    let is_open = rx!(ctx; *$open || *$ctx_open)?;

    let content_state = rx!(ctx; {
        if *$is_open {
            "open".to_string()
        } else {
            "closed".to_string()
        }
    })?;
    let content = div(div(stored
        .with(|children| children.clone())?
        .attr("data-slot", "popover-content")
        .attr("data-state", content_state)
        .attr("data-side", side_val)
        .attr("data-align", align_val)
        .attr("role", "dialog")
        .attr("tabindex", "-1")
        .class(content_cls)))
    .attr("data-radix-popper-content-wrapper", "")
    .class(wrapper_cls)
    .attr("style", wrapper_style);
    let portal = chain!(
        // Overlay for click-outside
        div(())
            .attr("data-slot", "popover-overlay")
            .class(tw!("fixed inset-0 z-50 bg-transparent"))
            .on(event::click, move |_| -> SilexResult<()> {
                context.close()?;
                on_close.invoke(())
            }),
        content,
    );

    Ok(Portal(ctx, is_open)
        .children(portal)
        .host_attrs(PortalHostAttrs::new().attr("data-portal-host", "popover")?)
        .build())
}

#[component]
pub fn PopoverTrigger<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain] context: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<'scope, ()>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let trigger_cls = rx!(ctx; {
        let base = tw!("inline-block cursor-pointer");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "popover-trigger")
        .class(trigger_cls)
        .on(
            event::click,
            move |e: web_sys::MouseEvent| -> SilexResult<()> {
                let target = e.current_target().or_else(|| e.target());
                if let Some(target) = target
                    && let Ok(el) = target.dyn_into::<web_sys::Element>()
                {
                    context.update_anchor_from_element(&el)?;
                }
                context.toggle()?;
                on_click.invoke(())
            },
        ))
}

#[component]
pub fn Popover<'scope, Ctx, C, V>(
    #[ctx] ctx: Ctx,
    children: C,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope>
where
    C: Fn(PopoverContext<'scope>) -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    let context = PopoverContext::new(owner)?;
    let root_cls = rx!(ctx; {
        let base = tw!("relative inline-block");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children(context))
        .attr("data-slot", "popover")
        .class(root_cls))
}
