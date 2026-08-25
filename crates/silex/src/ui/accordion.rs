use silex_core::prelude::*;
use silex_dom::attribute::{AttributeRequest, AttributeTarget, AttributeValue};
use silex_html::{button, div, path, svg};
use silex_macros::{component, tw};
use silex_view::prelude::*;
use silex_view::{MountAncestry, StableBranch};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ACCORDION_ITEM_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum AccordionRelation {
    Trigger,
    Content,
}

fn accordion_item(ancestry: &MountAncestry, dom: &DomContext) -> Option<DomElement> {
    ancestry.find_element(|element| {
        dom.get_attribute(element, "data-slot")
            .ok()
            .flatten()
            .as_deref()
            == Some("accordion-item")
    })
}

fn accordion_relation<'scope>(relation: AccordionRelation) -> AttrOp<'scope> {
    AttrOp::custom(move |element, context| {
        let element_for_task = element.clone();
        let ancestry = context.ancestry().clone();
        let dom = context.dom().clone();
        context.on_commit(move || {
            let Some(item) = accordion_item(&ancestry, &dom) else {
                return Ok(());
            };
            let Some(item_id) = dom.get_attribute(&item, "data-accordion-item-id")? else {
                return Ok(());
            };

            let (id, related_attribute, related_id) = match relation {
                AccordionRelation::Trigger => (
                    format!("{item_id}-trigger"),
                    "aria-controls",
                    format!("{item_id}-content"),
                ),
                AccordionRelation::Content => (
                    format!("{item_id}-content"),
                    "aria-labelledby",
                    format!("{item_id}-trigger"),
                ),
            };
            dom.set_attribute(AttributeRequest::new(
                &element_for_task,
                AttributeTarget::named("id"),
                AttributeValue::text(id),
            ))?;
            dom.set_attribute(AttributeRequest::new(
                &element_for_task,
                AttributeTarget::named(related_attribute),
                AttributeValue::text(related_id),
            ))?;
            Ok(())
        })?;

        let element_for_cleanup = element.clone();
        let related_attribute = match relation {
            AccordionRelation::Trigger => "aria-controls",
            AccordionRelation::Content => "aria-labelledby",
        };
        let dom = context.dom().clone();
        context.owner().on_cleanup(
            Box::new(move || -> SilexResult<()> {
                let _ = dom.set_attribute(AttributeRequest::new(
                    &element_for_cleanup,
                    AttributeTarget::named("id"),
                    AttributeValue::Removed,
                ));
                let _ = dom.set_attribute(AttributeRequest::new(
                    &element_for_cleanup,
                    AttributeTarget::named(related_attribute),
                    AttributeValue::Removed,
                ));
                Ok(())
            }),
            context.error_handler(),
        )?;
        Ok(())
    })
}

fn focus_event_target(event: &DomEvent) -> SilexResult<()> {
    event.focus_target()?;
    Ok(())
}

fn focus_content_trigger(
    content: &DomElement,
    ancestry: &MountAncestry,
    dom: &DomContext,
) -> SilexResult<()> {
    if let Some(item) = accordion_item(ancestry, dom) {
        let trigger = dom
            .children(item.node())?
            .into_iter()
            .filter_map(|node| dom.element(&node).ok())
            .find(|element| {
                dom.get_attribute(element, "data-slot")
                    .ok()
                    .flatten()
                    .as_deref()
                    == Some("accordion-trigger")
            });
        if let Some(trigger) = trigger {
            dom.focus(&trigger)?;
            return Ok(());
        }
    }

    if let Some(active) = dom.active_element().ok().flatten()
        && dom.contains(content, active.node()).unwrap_or(false)
    {
        let _ = active;
    }
    Ok(())
}

fn content_focus_binding<'scope>(open: Rx<'scope, bool>) -> AttrOp<'scope> {
    AttrOp::on_commit(move |element, context| {
        let content = element.clone();
        let ancestry = context.ancestry().clone();
        let dom = context.dom().clone();
        context.owner().effect_with_previous(
            EffectPhase::Normal,
            move |previous: Option<&bool>| -> SilexResult<bool> {
                let current = open.with(|value| *value)?;
                if previous == Some(&true)
                    && !current
                    && dom.active_element().ok().flatten().is_some_and(|active| {
                        dom.contains(&content, active.node()).unwrap_or(false)
                    })
                {
                    focus_content_trigger(&content, &ancestry, &dom)?;
                }
                Ok(current)
            },
            context.error_handler(),
        )
    })
}

fn unmount_content_slot<'scope>(
    children: AnyView<'scope>,
    open: Rx<'scope, bool>,
) -> AttrOp<'scope> {
    AttrOp::custom(move |_, context| {
        let branch_children = children.clone();
        let key_fn = move || open.with(|value| BranchEvaluation::new(*value, ()));
        let branch_fn = move |evaluation: BranchEvaluation<bool, ()>, _context| {
            if *evaluation.key() {
                branch_children.clone()
            } else {
                ().into_any()
            }
        };
        let branch = StableBranch::new(key_fn, branch_fn);
        context.mount_unit(&branch)
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccordionContentMode {
    #[default]
    KeepAlive,
    UnmountWhenClosed,
}

#[component]
pub fn Accordion<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
) -> impl View<'scope> {
    let root_cls = rx!(ctx; {
        let base = tw!("w-full");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    Ok(div(children).attr("data-slot", "accordion").class(root_cls))
}

#[component]
pub fn AccordionItem<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    value: &'static str,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
) -> impl View<'scope> {
    let item_cls = rx!(ctx; {
        let base = tw!("border-b border-slate-200 dark:border-slate-800 last:border-b-0");
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;
    let item_id = format!(
        "silex-accordion-item-{}",
        NEXT_ACCORDION_ITEM_ID.fetch_add(1, Ordering::Relaxed)
    );
    let item_id = owner.stored(item_id)?.with(Clone::clone)?;

    Ok(div(children)
        .attr("data-slot", "accordion-item")
        .attr("data-value", value)
        .attr("data-accordion-item-id", item_id)
        .class(item_cls))
}

#[component]
pub fn AccordionTrigger<'scope, Ctx>(
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
    on_click: Callback<'scope, ()>,
) -> impl View<'scope> {
    let trigger_cls = rx!(ctx; {
        let base = tw!(
            "flex flex-1 w-full items-center justify-between gap-4 py-4 text-left text-sm font-medium transition-all hover:underline cursor-pointer border-0 bg-transparent text-slate-900 dark:text-slate-100 [&[data-state=open]>svg]:rotate-180"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    let state_attr = rx!(ctx; if *$open { "open" } else { "closed" })?;
    let expanded_attr = rx!(ctx; if *$open { "true" } else { "false" })?;
    let icon_cls = rx!(ctx; {
        let is_open = *$open;
        if is_open {
            tw!("pointer-events-none size-4 shrink-0 translate-y-0.5 text-slate-500 dark:text-slate-400 transition-transform duration-200 rotate-180").to_string()
        } else {
            tw!("pointer-events-none size-4 shrink-0 translate-y-0.5 text-slate-500 dark:text-slate-400 transition-transform duration-200").to_string()
        }
    })?;

    let icon = svg(path().attr("d", "m6 9 6 6 6-6"))
        .attr("xmlns", "http://www.w3.org/2000/svg")
        .attr("width", "16")
        .attr("height", "16")
        .attr("viewBox", "0 0 24 24")
        .attr("fill", "none")
        .attr("stroke", "currentColor")
        .attr("stroke-width", "2")
        .attr("stroke-linecap", "round")
        .attr("stroke-linejoin", "round")
        .class(icon_cls);

    Ok(button(chain!(children, icon))
        .attr("data-slot", "accordion-trigger")
        .attr("data-state", state_attr)
        .attr("aria-expanded", expanded_attr)
        .apply(accordion_relation(AccordionRelation::Trigger))
        .class(trigger_cls)
        .on_click(move |event: DomEvent| -> SilexResult<()> {
            let was_open = open.with(|value| *value)?;
            on_click.invoke(())?;
            if was_open {
                focus_event_target(&event)?;
            }
            Ok(())
        }))
}

#[component]
pub fn AccordionContent<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    open: Rx<'scope, bool>,
    #[prop(into)]
    #[chain(default)]
    class: Rx<'scope, String>,
    #[chain(default)] mode: AccordionContentMode,
) -> impl View<'scope> {
    let content_cls = rx!(ctx; {
        let base = tw!(
            "overflow-hidden text-sm pb-4 pt-0 text-slate-600 dark:text-slate-400 transition-all data-[state=closed]:hidden"
        );
        let extra = $class;
        if extra.is_empty() {
            base.to_string()
        } else {
            format!("{} {}", base, extra)
        }
    })?;

    let state_attr = rx!(ctx; if *$open { "open" } else { "closed" })?;
    let aria_hidden = rx!(ctx; if *$open { "false" } else { "true" })?;
    let inert = rx!(ctx; !*$open)?;
    let content = match mode {
        AccordionContentMode::KeepAlive => div(children).apply(content_focus_binding(open)),
        AccordionContentMode::UnmountWhenClosed => div(())
            .apply(content_focus_binding(open))
            .apply(unmount_content_slot(children, open)),
    };

    Ok(content
        .attr("data-slot", "accordion-content")
        .attr("data-state", state_attr)
        .attr("aria-hidden", aria_hidden)
        .attr("inert", inert)
        .apply(accordion_relation(AccordionRelation::Content))
        .class(content_cls))
}
