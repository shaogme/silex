use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::span;
use silex_macros::{component, tw_variants};

#[component]
pub fn Badge<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let badge_variants = tw_variants! {
        base: "inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-full border border-solid border-transparent px-2.5 py-0.5 text-xs font-medium whitespace-nowrap transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&>svg]:pointer-events-none [&>svg]:size-3",
        variants: {
            variant: {
                default: "border-transparent bg-primary text-primary-foreground hover:bg-primary/90 shadow-xs",
                secondary: "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/90",
                destructive: "border-transparent bg-destructive text-white hover:bg-destructive/90 dark:bg-destructive/60 dark:hover:bg-destructive/90 shadow-xs",
                outline: "border-border text-foreground hover:bg-accent hover:text-accent-foreground",
                ghost: "hover:bg-accent hover:text-accent-foreground text-foreground",
                link: "text-primary underline-offset-4 hover:underline"
            }
        },
        default_variants: {
            variant: "default"
        }
    };

    let cls = rx!(ctx; {
        let base_cls = badge_variants.get($variant);
        let extra = $class;
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    Ok(span(children).class(cls))
}
