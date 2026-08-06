use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::button;
use silex_macros::{component, tw_variants};

#[component]
pub fn Button<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    variant: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    size: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let button_variants = tw_variants! {
        base: "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm font-medium whitespace-nowrap transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 cursor-pointer border-0 [&_svg]:pointer-events-none [&_svg]:shrink-0",
        variants: {
            variant: {
                default: "bg-primary text-primary-foreground hover:bg-primary/90 shadow-xs",
                destructive: "bg-destructive text-white hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:bg-destructive/60 dark:focus-visible:ring-destructive/40 shadow-xs",
                outline: "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:border-input dark:bg-input/30 dark:hover:bg-input/50",
                secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
                ghost: "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
                link: "text-primary underline-offset-4 hover:underline"
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

    let cls = rx!(scope; {
        let base_cls = button_variants.get($variant, $size);
        let extra = $class;
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    button(children).attr("data-slot", "button").class(cls)
}
