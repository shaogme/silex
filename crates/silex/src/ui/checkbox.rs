use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, path, svg};
use silex_macros::{component, tw};

#[component]
pub fn Checkbox<'scope>(
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    #[prop(into)]
    #[chain(default)]
    checked: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<'scope, bool>,
) -> impl View<'scope> {
    let cls = rx!(scope; error_handler; {
        let is_checked = *$checked;
        let base = if is_checked {
            tw!("peer size-4 shrink-0 rounded-[4px] border border-solid shadow-xs transition-all duration-150 ease-in-out focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 inline-flex items-center justify-center cursor-pointer bg-slate-900 text-slate-50 border-slate-900 dark:bg-slate-50 dark:text-slate-900 dark:border-slate-50").to_string()
        } else {
            tw!("peer size-4 shrink-0 rounded-[4px] border border-solid shadow-xs transition-all duration-150 ease-in-out focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 inline-flex items-center justify-center cursor-pointer border-slate-300 bg-white dark:border-slate-800 dark:bg-slate-900/30").to_string()
        };
        let extra = $class;
        if extra.is_empty() {
            base
        } else {
            format!("{} {}", base, extra)
        }
    });

    let check_icon = rx!(scope; error_handler; {
        let is_checked = *$checked;
        let icon_cls = if is_checked {
            tw!("size-3.5 transition-all duration-150 ease-in-out opacity-100 scale-100").to_string()
        } else {
            tw!("size-3.5 transition-all duration-150 ease-in-out opacity-0 scale-50").to_string()
        };

        svg(path().attr("d", "M20 6 9 17l-5-5"))
            .attr("xmlns", "http://www.w3.org/2000/svg")
            .attr("width", "14")
            .attr("height", "14")
            .attr("viewBox", "0 0 24 24")
            .attr("fill", "none")
            .attr("stroke", "currentColor")
            .attr("stroke-width", "3")
            .attr("stroke-linecap", "round")
            .attr("stroke-linejoin", "round")
            .class(icon_cls)
    });

    Ok(button(check_icon)
        .class(cls)
        .on_click(move |_| -> SilexResult<()> { on_change.invoke(!checked.get()?) }))
}
