use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, span};
use silex_macros::{component, tw};

#[component]
pub fn Switch<'scope>(
    scope: Scope<'scope>,
    #[prop(into)]
    #[chain(default)]
    checked: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    size: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_change: Callback<'scope, bool>,
) -> impl View<'scope> {
    let track_cls = rx!(scope; {
        let is_checked = *$checked;
        let is_sm = $size.as_str() == "sm";

        let base_cls = if is_sm {
            if is_checked {
                tw!("peer inline-flex h-3.5 w-6 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-colors duration-200 ease-in-out outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 bg-primary").to_string()
            } else {
                tw!("peer inline-flex h-3.5 w-6 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-colors duration-200 ease-in-out outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 bg-input dark:bg-input/80").to_string()
            }
        } else {
            if is_checked {
                tw!("peer inline-flex h-[1.15rem] w-8 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-colors duration-200 ease-in-out outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 bg-primary").to_string()
            } else {
                tw!("peer inline-flex h-[1.15rem] w-8 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-colors duration-200 ease-in-out outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 bg-input dark:bg-input/80").to_string()
            }
        };

        let extra = $class;
        if extra.is_empty() {
            base_cls
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    let thumb_cls = rx!(scope; {
        let is_checked = *$checked;
        let is_sm = $size.as_str() == "sm";

        if is_sm {
            if is_checked {
                tw!("pointer-events-none block size-3 rounded-full bg-background ring-0 transition-transform duration-200 ease-in-out translate-x-[calc(100%-2px)] dark:bg-primary-foreground").to_string()
            } else {
                tw!("pointer-events-none block size-3 rounded-full bg-background ring-0 transition-transform duration-200 ease-in-out translate-x-0 dark:bg-foreground").to_string()
            }
        } else {
            if is_checked {
                tw!("pointer-events-none block size-4 rounded-full bg-background ring-0 transition-transform duration-200 ease-in-out translate-x-[calc(100%-2px)] dark:bg-primary-foreground").to_string()
            } else {
                tw!("pointer-events-none block size-4 rounded-full bg-background ring-0 transition-transform duration-200 ease-in-out translate-x-0 dark:bg-foreground").to_string()
            }
        }
    });

    button(span(()).class(thumb_cls))
        .class(track_cls)
        .on_click(move |_| {
            let _ = on_change.invoke(!checked.get());
        })
}
