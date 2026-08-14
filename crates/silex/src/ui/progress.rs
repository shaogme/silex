use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, tw};

#[component]
pub fn Progress<'scope, Ctx>(
    #[context] context: Ctx,
    #[prop(into)]
    #[chain(default)]
    value: Signal<'scope, u32>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
) -> impl View<'scope> {
    let container_cls = rx!(context; {
        let base =
            tw!("relative h-2 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let indicator_style = rx!(context; {
        let pct = (*$value).min(100);
        format!("width: {}%;", pct)
    });

    Ok(div(div(())
        .class(tw!(
            "h-full bg-slate-900 dark:bg-slate-50 transition-all duration-300"
        ))
        .style(indicator_style))
    .class(container_cls))
}
