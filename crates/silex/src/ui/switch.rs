use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, span};
use silex_macros::{component, tw};

#[component]
pub fn Switch(
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
    let track_cls = rx!(move || {
        let is_checked = checked.get();
        let base = tw!(
            "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
            (
                is_checked,
                "bg-slate-900 dark:bg-slate-50",
                "bg-slate-200 dark:bg-slate-800"
            )
        ).get();
        let extra = class.get();
        if extra.is_empty() {
            base
        } else {
            format!("{} {}", base, extra)
        }
    });

    let thumb_cls = rx!(move || {
        let is_checked = checked.get();
        tw!(
            "pointer-events-none block size-4 rounded-full bg-white dark:bg-slate-950 shadow-md ring-0 transition-transform",
            (is_checked, "translate-x-4", "translate-x-0")
        ).get()
    });

    button(span(()).class(thumb_cls))
        .class(track_cls)
        .on_click(move |_| on_change.call(!checked.get()))
}
