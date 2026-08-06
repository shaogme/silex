use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_html::{button, div};
use silex_macros::{component, styled, tw};

styled! {
    pub DialogHeader<'scope><div>(children: AnyView<'scope>) {
        @apply flex flex-col gap-2 text-center sm:text-left;
    }
}

styled! {
    pub DialogTitle<'scope><h2>(children: AnyView<'scope>) {
        @apply text-lg leading-none font-semibold;
    }
}

styled! {
    pub DialogDescription<'scope><p>(children: AnyView<'scope>) {
        @apply text-sm text-slate-500 dark:text-slate-400;
    }
}

styled! {
    pub DialogFooter<'scope><div>(children: AnyView<'scope>) {
        @apply flex flex-col-reverse gap-2 sm:flex-row sm:justify-end;
    }
}

#[component]
pub fn Dialog<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    open: Signal<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Signal<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_close: Callback<'scope, ()>,
) -> impl View<'scope> {
    let content_cls = rx!(scope; {
        let base = tw!(
            "fixed left-[50%] top-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border border-solid border-slate-200 bg-white p-6 shadow-lg sm:max-w-lg dark:border-slate-800 dark:bg-slate-950 text-slate-950 dark:text-slate-50"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    });

    let stored_children = scope.stored(children);

    rx!(scope; {
        if *$open {
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
                     stored_children.with(|children| children.clone())
                 )).class(content_cls)
            )).into_any()
        } else {
            ().into_any()
        }
    })
}
