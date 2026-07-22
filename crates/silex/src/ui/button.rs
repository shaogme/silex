use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::button;
use silex_macros::{component, tw_variants};

#[component]
pub fn Button(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    size: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let button_variants = tw_variants! {
        base: "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm font-medium whitespace-nowrap transition-all outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 cursor-pointer border-0",
        variants: {
            variant: {
                default: "bg-slate-900 text-slate-50 hover:bg-slate-900/90 dark:bg-slate-50 dark:text-slate-900 dark:hover:bg-slate-50/90 shadow-xs",
                destructive: "bg-rose-600 text-white hover:bg-rose-600/90 dark:bg-rose-600 dark:hover:bg-rose-600/90 shadow-xs",
                outline: "border border-solid border-slate-200 bg-white hover:bg-slate-100 hover:text-slate-900 dark:border-slate-800 dark:bg-slate-950 dark:hover:bg-slate-800 dark:hover:text-slate-50 shadow-xs",
                secondary: "bg-slate-100 text-slate-900 hover:bg-slate-100/80 dark:bg-slate-800 dark:text-slate-50 dark:hover:bg-slate-800/80",
                ghost: "hover:bg-slate-100 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-slate-50",
                link: "text-slate-900 underline-offset-4 hover:underline dark:text-slate-50"
            },
            size: {
                default: "h-9 px-4 py-2",
                xs: "h-6 rounded-md px-2 text-xs gap-1",
                sm: "h-8 rounded-md px-3 text-xs gap-1.5",
                lg: "h-10 rounded-md px-6",
                icon: "size-9 p-0",
                "icon-xs": "size-6 rounded-md p-0",
                "icon-sm": "size-8 p-0",
                "icon-lg": "size-10 p-0"
            }
        },
        default_variants: {
            variant: "default",
            size: "default"
        }
    };

    let cls = rx!(move || {
        let v = variant.get();
        let s = size.get();
        let base_cls = button_variants.get(
            if v.is_empty() { "default" } else { v.as_str() },
            if s.is_empty() { "default" } else { s.as_str() },
        );
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    button(children).class(cls)
}
