use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::button;
use silex_macros::{component, tw_variants};

#[component]
pub fn Toggle(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    pressed: Signal<bool>,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    size: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<bool>,
) -> impl View {
    let toggle_variants = tw_variants! {
        base: "inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium whitespace-nowrap transition-all outline-none hover:bg-muted hover:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 cursor-pointer border-0 data-[state=on]:bg-accent data-[state=on]:text-accent-foreground dark:data-[state=on]:bg-slate-800 dark:data-[state=on]:text-slate-50",
        variants: {
            variant: {
                default: "bg-transparent",
                outline: "border border-input bg-transparent shadow-xs hover:bg-accent hover:text-accent-foreground dark:border-slate-800"
            },
            size: {
                default: "h-9 min-w-9 px-2",
                sm: "h-8 min-w-8 px-1.5 text-xs",
                lg: "h-10 min-w-10 px-2.5"
            }
        },
        default_variants: {
            variant: "default",
            size: "default"
        }
    };

    let cls = rx!(move || {
        let base_cls = toggle_variants.get(variant.get(), size.get());
        let extra = class.get();
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    let state_attr = rx!(move || if pressed.get() { "on" } else { "off" });

    button(children)
        .attr("data-slot", "toggle")
        .attr("aria-pressed", rx!(move || pressed.get().to_string()))
        .attr("data-state", state_attr)
        .class(cls)
        .on_click(move |_| on_change.call(!pressed.get()))
}
