use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, div};
use silex_macros::{component, tw, tw_variants};

#[component]
pub fn Tabs<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    orientation: Rx<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
) -> impl View<'scope> {
    let cls = rx!(ctx; {
        let base = tw!("group/tabs flex gap-2 data-[orientation=horizontal]:flex-col");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    let orient = rx!(ctx; {
        let o = $orientation;
        if o.is_empty() {
            "horizontal".to_string()
        } else {
            o.clone()
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "tabs")
        .attr("data-orientation", orient)
        .attr("orientation", orient)
        .class(cls))
}

#[component]
pub fn TabsList<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    variant: Rx<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
) -> impl View<'scope> {
    let list_variants = tw_variants! {
        base: "group/tabs-list inline-flex w-fit items-center justify-center rounded-lg p-[3px] text-muted-foreground group-data-[orientation=horizontal]/tabs:h-9 group-data-[orientation=vertical]/tabs:h-fit group-data-[orientation=vertical]/tabs:flex-col data-[variant=line]:rounded-none",
        variants: {
            variant: {
                default: "bg-muted",
                line: "gap-1 bg-transparent"
            }
        },
        default_variants: {
            variant: "default"
        }
    };

    let cls = rx!(ctx; {
        let base_cls = list_variants.get($variant);
        let extra = $class;
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    })?;

    let var_attr = rx!(ctx; {
        let v = $variant;
        if v.is_empty() {
            "default".to_string()
        } else {
            v.clone()
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "tabs-list")
        .attr("data-variant", var_attr)
        .class(cls))
}

#[component]
pub fn TabsTrigger<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    value: &'static str,
    #[prop(into)]
    #[chain(default)]
    active_tab: Rx<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_select: Callback<'scope, &'static str>,
) -> impl View<'scope> {
    let cls = rx!(ctx; {
        let base = tw!(
            "relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center gap-1.5 rounded-md border border-transparent px-2 py-1 text-sm font-medium whitespace-nowrap text-foreground/60 transition-all group-data-[orientation=vertical]/tabs:w-full group-data-[orientation=vertical]/tabs:justify-start hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50 group-data-[variant=default]/tabs-list:data-[state=active]:shadow-sm group-data-[variant=line]/tabs-list:data-[state=active]:shadow-none dark:text-muted-foreground dark:hover:text-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent dark:group-data-[variant=line]/tabs-list:data-[state=active]:border-transparent dark:group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent data-[state=active]:bg-background data-[state=active]:text-foreground dark:data-[state=active]:border-input dark:data-[state=active]:bg-input/30 dark:data-[state=active]:text-foreground after:absolute after:bg-foreground after:opacity-0 after:transition-opacity group-data-[orientation=horizontal]/tabs:after:inset-x-0 group-data-[orientation=horizontal]/tabs:after:bottom-[-5px] group-data-[orientation=horizontal]/tabs:after:h-0.5 group-data-[orientation=vertical]/tabs:after:inset-y-0 group-data-[orientation=vertical]/tabs:after:-right-1 group-data-[orientation=vertical]/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-[state=active]:after:opacity-100"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    let state_attr = rx!(ctx; {
        if $active_tab.as_str() == value {
            "active"
        } else {
            "inactive"
        }
    })?;

    Ok(button(children)
        .attr("data-slot", "tabs-trigger")
        .attr("data-state", state_attr)
        .attr("data-value", value)
        .class(cls)
        .on_click(move |_| -> SilexResult<()> { on_select.invoke(value) }))
}

#[component]
pub fn TabsContent<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    value: &'static str,
    #[prop(into)]
    #[chain(default)]
    active_tab: Rx<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
) -> impl View<'scope> {
    let cls = rx!(ctx; {
        let base = tw!("flex-1 outline-none");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    let state_attr = rx!(ctx; {
        let val = value;
        if !val.is_empty() && $active_tab.as_str() == val {
            "active"
        } else if !val.is_empty() {
            "inactive"
        } else {
            "active"
        }
    })?;

    Ok(div(children)
        .attr("data-slot", "tabs-content")
        .attr("data-state", state_attr)
        .class(cls))
}
