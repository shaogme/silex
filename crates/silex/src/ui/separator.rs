use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::{component, tw};

#[component]
pub fn Separator(
    #[prop(into)]
    #[chain(default)]
    orientation: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
) -> impl View {
    let cls = rx!(move || {
        let orient = orientation.get();
        let base_cls = if orient == "vertical" {
            tw!("shrink-0 bg-slate-200 dark:bg-slate-800 h-full w-[1px]")
        } else {
            tw!("shrink-0 bg-slate-200 dark:bg-slate-800 h-[1px] w-full")
        };
        let extra = class.get();
        if extra.is_empty() {
            base_cls.to_string()
        } else {
            format!("{} {}", base_cls, extra)
        }
    });

    div(()).class(cls)
}
