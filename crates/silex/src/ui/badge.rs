use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::span;
use silex_macros::{component, tw_variants};

#[component]
pub fn Badge(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let badge_variants = tw_variants! {
        base: "inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-full border border-solid border-transparent px-2.5 py-0.5 text-xs font-medium whitespace-nowrap transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2",
        variants: {
            variant: {
                default: "border-transparent bg-slate-900 text-slate-50 hover:bg-slate-900/90 dark:bg-slate-50 dark:text-slate-900 dark:hover:bg-slate-50/90 shadow-xs",
                secondary: "border-transparent bg-slate-100 text-slate-900 hover:bg-slate-100/90 dark:bg-slate-800 dark:text-slate-50 dark:hover:bg-slate-800/90",
                destructive: "border-transparent bg-rose-600 text-white dark:bg-rose-900 dark:text-slate-50 dark:hover:bg-rose-900/90 shadow-xs",
                outline: "border-slate-200 text-slate-950 dark:border-slate-800 dark:text-slate-50",
                ghost: "hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-900 dark:text-slate-50",
                link: "text-slate-900 underline-offset-4 hover:underline dark:text-slate-50"
            }
        },
        default_variants: {
            variant: "default"
        }
    };

    let cls = rx!(move || {
        let v = variant.get();
        let base_cls = badge_variants.get(if v.is_empty() { "default" } else { v.as_str() });
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    span(children).class(cls)
}
