use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, div, path, svg};
use silex_macros::{component, tw};

#[component]
pub fn Accordion<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let root_cls = rx!(ctx; {
        let base = tw!("w-full");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children).attr("data-slot", "accordion").class(root_cls))
}

#[component]
pub fn AccordionItem<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    value: &'static str,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let item_cls = rx!(ctx; {
        let base = tw!("border-b border-slate-200 dark:border-slate-800 last:border-b-0");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    Ok(div(children)
        .attr("data-slot", "accordion-item")
        .attr("data-value", value)
        .class(item_cls))
}

#[component]
pub fn AccordionTrigger<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    open: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_click: Callback<'scope, ()>,
) -> impl View<'scope> {
    let trigger_cls = rx!(ctx; {
        let base = tw!(
            "flex flex-1 w-full items-center justify-between gap-4 py-4 text-left text-sm font-medium transition-all hover:underline cursor-pointer border-0 bg-transparent text-slate-900 dark:text-slate-100 [&[data-state=open]>svg]:rotate-180"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let state_attr = rx!(ctx; if *$open { "open" } else { "closed" });
    let icon_cls = rx!(ctx; {
        let is_open = *$open;
        if is_open {
            tw!("pointer-events-none size-4 shrink-0 translate-y-0.5 text-slate-500 dark:text-slate-400 transition-transform duration-200 rotate-180").to_string()
        } else {
            tw!("pointer-events-none size-4 shrink-0 translate-y-0.5 text-slate-500 dark:text-slate-400 transition-transform duration-200").to_string()
        }
    });

    let icon = svg(path().attr("d", "m6 9 6 6 6-6"))
        .attr("xmlns", "http://www.w3.org/2000/svg")
        .attr("width", "16")
        .attr("height", "16")
        .attr("viewBox", "0 0 24 24")
        .attr("fill", "none")
        .attr("stroke", "currentColor")
        .attr("stroke-width", "2")
        .attr("stroke-linecap", "round")
        .attr("stroke-linejoin", "round")
        .class(icon_cls);

    Ok(button(chain!(children, icon))
        .attr("data-slot", "accordion-trigger")
        .attr("data-state", state_attr)
        .class(trigger_cls)
        .on_click(move |_| -> SilexResult<()> { on_click.invoke(()) }))
}

#[component]
pub fn AccordionContent<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    open: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let content_cls = rx!(ctx; {
        let base = tw!(
            "overflow-hidden text-sm pb-4 pt-0 text-slate-600 dark:text-slate-400 transition-all"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let stored = owner.stored(children)?;

    Ok(rx!(ctx; {
        if *$open {
            div(stored.with(|children| children.clone()))
                .attr("data-slot", "accordion-content")
                .attr("data-state", "open")
                .class($content_cls)
                .into_any()
        } else {
            ().into_any()
        }
    }))
}
