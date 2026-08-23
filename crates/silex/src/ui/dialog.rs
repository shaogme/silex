use crate::components::{Portal, PortalHostAttrs};
use silex_core::prelude::*;
use silex_dom::document;
use silex_dom::prelude::*;
use silex_html::{button, div};
use silex_macros::{component, styled, tw};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement};

styled! {
    pub DialogHeader<'scope, Ctx><div>(#[ctx] ctx: Ctx, children: AnyView<'scope>) {
        @apply flex flex-col gap-2 text-center sm:text-left;
    }
}

styled! {
    pub DialogTitle<'scope, Ctx><h2>(#[ctx] ctx: Ctx, children: AnyView<'scope>) {
        @apply text-lg leading-none font-semibold;
    }
}

styled! {
    pub DialogDescription<'scope, Ctx><p>(#[ctx] ctx: Ctx, children: AnyView<'scope>) {
        @apply text-sm text-slate-500 dark:text-slate-400;
    }
}

styled! {
    pub DialogFooter<'scope, Ctx><div>(#[ctx] ctx: Ctx, children: AnyView<'scope>) {
        @apply flex flex-col-reverse gap-2 sm:flex-row sm:justify-end;
    }
}

fn dialog_focus_binding<'scope>(
    open: Rx<'scope, bool>,
    previous_focus: StoredValue<'scope, Option<Element>>,
) -> AttrOp<'scope> {
    AttrOp::on_commit(move |element, context| {
        let dialog = element.clone();
        context.owner().effect(
            EffectPhase::PostFlush,
            Box::new(move || -> SilexResult<()> {
                if open.with(|value| *value)? {
                    if silex_core::RxReadRef::with(&previous_focus, |value| value.is_none())? {
                        previous_focus.set(document().active_element())?;
                    }
                    let dialog = dialog.dyn_ref::<HtmlElement>().ok_or_else(|| {
                        SilexError::fatal(SilexErrorKind::Dom(
                            "Dialog content must be an HTML element".to_string(),
                        ))
                    })?;
                    dialog.focus().map_err(SilexError::fatal)?;
                } else if let Some(previous) =
                    silex_core::RxReadRef::with(&previous_focus, |value| value.clone())?
                {
                    if let Some(previous) = previous.dyn_ref::<HtmlElement>() {
                        previous.focus().map_err(SilexError::fatal)?;
                    }
                    previous_focus.set(None)?;
                }
                Ok(())
            }),
            context.error_handler(),
        )
    })
}

#[component]
pub fn Dialog<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    open: Rx<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
    #[prop(into)]
    #[chain(default)]
    on_close: Callback<'scope, ()>,
) -> impl View<'scope> {
    let content_cls = rx!(ctx; {
        let base = tw!(
            "fixed left-[50%] top-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border border-solid border-slate-200 bg-white p-6 shadow-lg sm:max-w-lg dark:border-slate-800 dark:bg-slate-950 text-slate-950 dark:text-slate-50"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    let stored_children = owner.stored(children)?;
    let previous_focus = owner.stored(None::<Element>)?;

    let portal = chain!(
        // Overlay 遮罩
        div(())
            .attr("data-slot", "dialog-overlay")
            .class(tw!("fixed inset-0 z-50 bg-black/50 backdrop-blur-xs"))
            .on_click(move |_| -> SilexResult<()> { on_close.invoke(()) }),
        // Content 窗口实体
        div(chain!(
            button("✕")
                .class(tw!("absolute right-4 top-4 rounded-sm opacity-70 transition-opacity hover:opacity-100 focus:outline-none cursor-pointer border-0 bg-transparent text-slate-500 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-50"))
                .on_click(move |_| -> SilexResult<()> { on_close.invoke(()) }),
            stored_children.with(|children| children.clone())?
        ))
        .attr("data-slot", "dialog-content")
        .attr("role", "dialog")
        .attr("aria-modal", "true")
        .attr("tabindex", "-1")
        .apply(dialog_focus_binding(open, previous_focus))
        .class(content_cls),
    );

    Ok(Portal(ctx, open)
        .children(portal)
        .host_attrs(PortalHostAttrs::new().attr("data-portal-host", "dialog")?)
        .build())
}
