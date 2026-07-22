use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, styled, tw_variants};

styled! {
    pub AlertTitle<h5>(children: AnyView) {
        @apply font-medium leading-none tracking-tight mb-1;
    }
}

styled! {
    pub AlertDescription<div>(children: AnyView) {
        @apply text-sm text-muted-foreground [&_p]:leading-relaxed;
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
        base: "relative w-full rounded-lg border border-solid p-4 text-sm [&>svg]:size-4 [&>svg]:translate-y-0.5 [&>svg]:text-current",
        variants: {
            variant: {
                default: "border-border bg-background text-foreground",
                destructive: "border-destructive/50 text-destructive bg-destructive/10 dark:border-destructive dark:bg-destructive/20"
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
