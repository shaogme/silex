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
    size: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<bool>,
) -> impl View {
    let track_cls = rx!(move || {
        let is_checked = checked.get();
        let s = size.get();
        let is_sm = s == "sm";

        let base_cls = if is_sm {
            tw!(
                "peer inline-flex h-3.5 w-6 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
                (
                    is_checked,
                    "bg-primary",
                    "bg-input dark:bg-input/80"
                )
            ).get()
        } else {
            tw!(
                "peer inline-flex h-[1.15rem] w-8 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
                (
                    is_checked,
                    "bg-primary",
                    "bg-input dark:bg-input/80"
                )
            ).get()
        };

        let extra = class.get();
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    let thumb_cls = rx!(move || {
        let is_checked = checked.get();
        let s = size.get();
        let is_sm = s == "sm";

        if is_sm {
            tw!(
                "pointer-events-none block size-3 rounded-full bg-background ring-0 transition-transform",
                (
                    is_checked,
                    "translate-x-[calc(100%-2px)] dark:bg-primary-foreground",
                    "translate-x-0 dark:bg-foreground"
                )
            ).get()
        } else {
            tw!(
                "pointer-events-none block size-4 rounded-full bg-background ring-0 transition-transform",
                (
                    is_checked,
                    "translate-x-[calc(100%-2px)] dark:bg-primary-foreground",
                    "translate-x-0 dark:bg-foreground"
                )
            ).get()
        }
    });

    button(span(()).class(thumb_cls))
        .class(track_cls)
        .on_click(move |_| on_change.call(!checked.get()))
}
