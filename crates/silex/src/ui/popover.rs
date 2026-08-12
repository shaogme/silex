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
    pub fn new(scope: Scope<'scope>) -> SilexResult<Self> {
        Ok(Self {
            open: scope.rw_signal(false)?,
            anchor_rect: scope.rw_signal((0.0, 0.0, 0.0, 0.0))?,
            content_height: scope.rw_signal(0.0)?,
        })
    }

    pub fn update_anchor_from_element(&self, el: &web_sys::Element) -> ReactiveResult<()> {
        let rect = el.get_bounding_client_rect();
        self.anchor_rect
            .set((rect.top(), rect.left(), rect.width(), rect.height()))
    }

    pub fn update_content_from_element(&self, el: &web_sys::Element) -> ReactiveResult<()> {
        let rect = el.get_bounding_client_rect();
        if rect.height() > 0.0 {
            self.content_height.set(rect.height())
        } else {
            Ok(())
        }
    }

    pub fn open(&self) -> ReactiveResult<()> {
        self.open.set(true)
    }

    pub fn close(&self) -> ReactiveResult<()> {
        self.open.set(false)
    }

    pub fn toggle(&self) -> ReactiveResult<()> {
        self.open.update(|v| *v = !*v)
    }
}

#[component]
pub fn PopoverHeader<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let header_cls = rx!(scope; error_handler; {
        let base = tw!("flex flex-col gap-1 text-sm");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children)
        .attr("data-slot", "popover-header")
        .class(header_cls))
}

#[component]
pub fn PopoverTitle<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let title_cls = rx!(scope; error_handler; {
        let base = tw!("font-medium leading-none");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children)
        .attr("data-slot", "popover-title")
        .class(title_cls))
}

#[component]
pub fn PopoverDescription<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let desc_cls = rx!(scope; error_handler; {
        let base = tw!("text-muted-foreground text-sm m-0");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(p(children)
        .attr("data-slot", "popover-description")
        .class(desc_cls))
}

#[component]
pub fn PopoverPortal<'scope>(children: AnyView<'scope>) -> impl View<'scope> {
    crate::components::Portal(children).build()
}

#[component]
pub fn PopoverAnchor<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[chain] ctx: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let anchor_cls = rx!(scope; error_handler; {
        let base = tw!("inline-block");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

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
                    ctx.update_anchor_from_element(&el)?;
                }
                Ok(())
            },
        ))
}

#[component]
pub fn PopoverClose<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[chain] ctx: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<'scope, ()>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let close_cls = rx!(scope; error_handler; {
        let base = tw!("inline-flex items-center justify-center cursor-pointer");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children)
        .attr("data-slot", "popover-close")
        .class(close_cls)
        .on(event::click, move |_| -> SilexResult<()> {
            ctx.close()?;
            on_click.invoke(())
        }))
}

#[component]
pub fn PopoverContent<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[chain] ctx: PopoverContext<'scope>,
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
    let side_val = rx!(scope; error_handler; {
        let s = (*$side).clone();
        if s.is_empty() {
            "bottom".to_string()
        } else {
            s
        }
    });

    let align_val = rx!(scope; error_handler; {
        let a = (*$align).clone();
        if a.is_empty() {
            "center".to_string()
        } else {
            a
        }
    });

    let offset_val = rx!(scope; error_handler; {
        let o = *$side_offset;
        if o == 0.0 { 4.0 } else { o }
    });

    let wrapper_cls =
        tw!("fixed left-0 top-0 z-50 min-w-max will-change-transform pointer-events-none");

    let anchor_rect = ctx.anchor_rect;
    let wrapper_style = rx!(scope; error_handler; {
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
    });

    let content_cls = rx!(scope; error_handler; {
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
    });

    let stored = scope.stored(children)?;
    let ctx_open = ctx.open;
    let is_open = rx!(scope; error_handler; *$open || *$ctx_open);

    Ok(rx!(scope; error_handler; {
        if *$is_open {
            crate::components::Portal(chain!(
                // Overlay for click-outside
                div(()).class(tw!("fixed inset-0 z-50 bg-transparent")).on(
                    event::click,
                    move |_| -> SilexResult<()> {
                        ctx.close()?;
                        on_close.invoke(())
                    }
                ),
                // Content wrapper
                div(div(stored.with(|children| children.clone())?
                    .attr("data-slot", "popover-content")
                    .attr("data-state", "open")
                    .attr("data-side", side_val)
                    .attr("data-align", align_val)
                    .attr("role", "dialog")
                    .attr("tabindex", "-1")
                    .class(content_cls))
                .attr("data-radix-popper-content-wrapper", "")
                .class(wrapper_cls)
                .attr("style", wrapper_style)
            ))).build()
            .into_any()
        } else {
            ().into_any()
        }
    }))
}

#[component]
pub fn PopoverTrigger<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: AnyView<'scope>,
    #[chain] ctx: PopoverContext<'scope>,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<'scope, ()>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let trigger_cls = rx!(scope; error_handler; {
        let base = tw!("inline-block cursor-pointer");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

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
                    ctx.update_anchor_from_element(&el)?;
                }
                ctx.toggle()?;
                on_click.invoke(())
            },
        ))
}

#[component]
pub fn Popover<'scope, C, V>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    children: C,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope>
where
    C: Fn(PopoverContext<'scope>) -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    let ctx = PopoverContext::new(scope)?;
    let root_cls = rx!(scope; error_handler; {
        let base = tw!("relative inline-block");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children(ctx))
        .attr("data-slot", "popover")
        .class(root_cls))
}
