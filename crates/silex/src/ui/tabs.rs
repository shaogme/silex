use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::button;
use silex_macros::{component, styled, tw};

styled! {
    pub TabsList<div>(children: AnyView) {
        @apply inline-flex h-9 items-center justify-center rounded-lg bg-slate-100 p-1 text-slate-500 dark:bg-slate-800 dark:text-slate-400;
    }
}

styled! {
    pub TabsContent<div>(children: AnyView) {
        @apply flex-1 outline-none mt-2;
    }
}

#[component]
pub fn TabsTrigger(
    children: AnyView,
    value: &'static str,
    #[prop(into)]
    #[chain(default)]
    active_tab: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_select: Callback<&'static str>,
) -> impl View {
    let cls = rx!(move || {
        let is_active = active_tab.get() == value;
        let base = tw!(
            "inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 cursor-pointer border-0",
            (
                is_active,
                "bg-white text-slate-950 shadow-xs dark:bg-slate-950 dark:text-slate-50 font-semibold",
                "text-slate-500 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-100"
            )
        ).get();
        let extra = class.get();
        if extra.is_empty() {
            base
        } else {
            format!("{} {}", base, extra)
        }
    });

    button(children)
        .class(cls)
        .on_click(move |_| on_select.call(value))
}
