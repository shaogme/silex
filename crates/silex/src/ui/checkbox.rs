use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::button;
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
            "peer size-4 shrink-0 rounded-sm border border-solid shadow-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 inline-flex items-center justify-center cursor-pointer",
            (
                is_checked,
                "bg-slate-900 text-slate-50 border-slate-900 dark:bg-slate-50 dark:text-slate-900 dark:border-slate-50",
                "border-slate-300 bg-white dark:border-slate-800 dark:bg-slate-950"
            )
        ).get();
        let extra = class.get();
        if extra.is_empty() {
            base
        } else {
            format!("{} {}", base, extra)
        }
    });

    let check_icon = rx!(move || if checked.get() { "✓" } else { "" } );

    button(check_icon)
        .class(cls)
        .on_click(move |_| on_change.call(!checked.get()))
}
