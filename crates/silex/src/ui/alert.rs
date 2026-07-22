use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, styled, tw_variants};

styled! {
    pub AlertTitle<h5>(children: AnyView) {
        @apply mb-1 font-medium leading-none tracking-tight;
    }
}

styled! {
    pub AlertDescription<div>(children: AnyView) {
        @apply text-sm text-slate-500 dark:text-slate-400;
    }
}

#[component]
pub fn Alert(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let alert_variants = tw_variants! {
        base: "relative w-full rounded-lg border border-solid p-4 text-sm",
        variants: {
            variant: {
                default: "border-slate-200 bg-white text-slate-950 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-50",
                destructive: "border-rose-500/50 bg-rose-50 text-rose-900 dark:border-rose-500/50 dark:bg-rose-950/50 dark:text-rose-200"
            }
        },
        default_variants: {
            variant: "default"
        }
    };

    let cls = rx!(move || {
        let v = variant.get();
        let base_cls = alert_variants.get(if v.is_empty() { "default" } else { v.as_str() });
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    div(children).class(cls)
}
