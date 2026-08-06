use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, styled, tw_variants};

styled! {
    pub AlertTitle<'scope><div>(children: AnyView<'scope>) {
        @apply col-start-2 line-clamp-1 min-h-4 font-medium tracking-tight;
    }
}

styled! {
    pub AlertDescription<'scope><div>(children: AnyView<'scope>) {
        @apply col-start-2 grid justify-items-start gap-1 text-sm text-muted-foreground [&_p]:leading-relaxed;
    }
}

#[component]
pub fn Alert<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let alert_variants = tw_variants! {
        base: "relative grid w-full grid-cols-[0_1fr] items-start gap-y-0.5 rounded-lg border px-4 py-3 text-sm has-[>svg]:grid-cols-[calc(var(--spacing)*4)_1fr] has-[>svg]:gap-x-3 [&>svg]:size-4 [&>svg]:translate-y-0.5 [&>svg]:text-current",
        variants: {
            variant: {
                default: "bg-card text-card-foreground",
                destructive: "bg-card text-destructive *:data-[slot=alert-description]:text-destructive/90 [&>svg]:text-current"
            }
        },
        default_variants: {
            variant: "default"
        }
    };

    let cls = rx!(scope; {
        let base_cls = alert_variants.get($variant);
        let extra = $class;
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    div(children).class(cls)
}
