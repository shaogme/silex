use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, div};
use silex_macros::{component, styled, tw};

styled! {
    pub DialogHeader<div>(children: AnyView) {
        @apply flex flex-col gap-2 text-center sm:text-left;
    }
}

styled! {
    pub DialogTitle<h2>(children: AnyView) {
        @apply text-lg leading-none font-semibold;
    }
}

styled! {
    pub DialogDescription<p>(children: AnyView) {
        @apply text-sm text-slate-500 dark:text-slate-400;
    }
}

styled! {
    pub DialogFooter<div>(children: AnyView) {
        @apply flex flex-col-reverse gap-2 sm:flex-row sm:justify-end;
    }
}

#[component]
pub fn Dialog(
    children: AnyView,
    #[prop(into)]
    #[chain(default)]
    open: Signal<bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<String>,
    #[prop(into)]
    #[chain(default)]
    on_close: Callback<()>,
) -> impl View {
    let content_cls = rx!(move || {
        let base = tw!(
            "fixed left-[50%] top-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border border-solid border-slate-200 bg-white p-6 shadow-lg sm:max-w-lg dark:border-slate-800 dark:bg-slate-950 text-slate-950 dark:text-slate-50"
        );
        let extra = class.get();
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let stored_children = StoredValue::new(children);

    rx!(move || {
        if open.get() {
            crate::components::Portal(view_chain!(
                // Overlay 遮罩
                div(())
                    .class(tw!("fixed inset-0 z-50 bg-black/50 backdrop-blur-xs"))
                    .on_click(move |_| {
                        let _ = on_close.invoke(());
                    }),
                // Content 窗口实体
                div(view_chain!(
                    button("✕")
                        .class(tw!("absolute right-4 top-4 rounded-sm opacity-70 transition-opacity hover:opacity-100 focus:outline-none cursor-pointer border-0 bg-transparent text-slate-500 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-50"))
                    .on_click(move |_| {
                        let _ = on_close.invoke(());
                    }),
                    stored_children.get()
                )).class(content_cls)
            )).into_any()
        } else {
            ().into_any()
        }
    })
}
