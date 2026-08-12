use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, tw};

#[component]
pub fn Separator<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    #[prop(into)]
    #[chain(default)]
    orientation: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let cls = rx!(scope; error_handler; {
        let orient = $orientation;
        let base_cls = if orient.as_str() == "vertical" {
            tw!("shrink-0 bg-slate-200 dark:bg-slate-800 h-full w-[1px]")
        } else {
            tw!("shrink-0 bg-slate-200 dark:bg-slate-800 h-[1px] w-full")
        };
        let extra = $class;
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    Ok(div(()).class(cls))
}
