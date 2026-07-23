use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, path, svg};
use silex_macros::{component, tw};

#[component]
pub fn Checkbox(
    #[prop(into)]
    #[chain(default)]
    checked: Signal<bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<bool>,
) -> impl View {
    let cls = rx!(move || {
        let is_checked = checked.get();
        let base = tw!(
            "peer size-4 shrink-0 rounded-[4px] border border-solid shadow-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 inline-flex items-center justify-center cursor-pointer",
            (
                is_checked,
                "bg-slate-900 text-slate-50 border-slate-900 dark:bg-slate-50 dark:text-slate-900 dark:border-slate-50",
                "border-slate-300 bg-white dark:border-slate-800 dark:bg-slate-900/30"
            )
        ).get();
        let extra = class.get();
        if extra.is_empty() {
            base
        } else {
            format!("{} {}", base, extra)
        }
    });

    let check_icon = rx!(move || {
        if checked.get() {
            let icon_cls = tw!("size-3.5");
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
                .class(icon_cls.to_string())
                .into_any()
        } else {
            ().into_any()
        }
    });

    button(check_icon)
        .class(cls)
        .on_click(move |_| on_change.call(!checked.get()))
}
