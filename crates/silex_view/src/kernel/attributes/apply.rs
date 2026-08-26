use super::{
    binding::ReactiveBindingTarget,
    dom::{apply_attr_target, parse_style, set_classes},
    model::{ApplyTarget, Attr, AttrData, CombinedClasses, CombinedStyles},
};
use crate::{
    kernel::MountContext,
    lifecycle::{MountErrorHandler, MountOwnerToken},
};
use silex_core::{EffectPhase, RxGet, SilexResult};
use silex_dom::model::{
    DomElement,
    attribute::{AttributeRequest, AttributeTarget, AttributeValue},
};
use std::collections::{BTreeMap, HashSet};
pub(crate) fn apply_update<'scope>(
    element: &DomElement,
    target: ApplyTarget,
    data: AttrData<'scope>,
    owner: &MountOwnerToken<'scope>,
    context: &MountContext<'scope>,
    handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    match data {
        AttrData::StaticAttr(value) => apply_attr_target(context.dom(), element, &target, &value),
        AttrData::ReactiveAttr(rx) => {
            let element = element.clone();
            let dom = context.dom().clone();
            let target = target.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || apply_attr_target(&dom, &element, &target, &rx.get()?)),
                handler,
            )
        }
        AttrData::ReactiveString(rx) => {
            let element = element.clone();
            let dom = context.dom().clone();
            let target = target.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || {
                    apply_attr_target(&dom, &element, &target, &Attr::from(rx.get()?))
                }),
                handler,
            )
        }
        AttrData::ReactiveBool(rx) => {
            let element = element.clone();
            let dom = context.dom().clone();
            let target = target.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || {
                    apply_attr_target(&dom, &element, &target, &Attr::from(rx.get()?))
                }),
                handler,
            )
        }
        AttrData::ReactiveOptionString(rx) => {
            let element = element.clone();
            let dom = context.dom().clone();
            let target = target.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || {
                    apply_attr_target(&dom, &element, &target, &Attr::from(rx.get()?))
                }),
                handler,
            )
        }
    }
}

pub(crate) fn apply_classes<'scope>(
    element: &DomElement,
    value: CombinedClasses<'scope>,
    owner: &MountOwnerToken<'scope>,
    context: &MountContext<'scope>,
    handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let (statics, toggles, reactives) = value.into_parts();
    let statics = statics
        .iter()
        .flat_map(|item| item.split_whitespace().map(ToOwned::to_owned))
        .collect::<HashSet<_>>();
    let static_for_effect = statics.clone();
    let element_for_effect = element.clone();
    let dom = context.dom().clone();
    owner.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let mut next = static_for_effect.clone();
            for (name, plan) in &toggles {
                if plan.bool_value()? {
                    next.insert(name.to_string());
                }
            }
            for plan in &reactives {
                next.extend(
                    plan.string_value()?
                        .split_whitespace()
                        .map(ToOwned::to_owned),
                );
            }
            set_classes(&dom, &element_for_effect, &next)
        }),
        handler,
    )?;
    let dom_for_cleanup = context.dom().clone();
    let element_for_cleanup = element.clone();
    owner.on_cleanup(
        Box::new(move || set_classes(&dom_for_cleanup, &element_for_cleanup, &statics)),
        handler,
    )
}

pub(crate) fn apply_styles<'scope>(
    element: &DomElement,
    value: CombinedStyles<'scope>,
    owner: &MountOwnerToken<'scope>,
    context: &MountContext<'scope>,
    handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let (statics, properties, sheets) = value.into_parts();
    let static_values = statics
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
    let static_for_cleanup = static_values.clone();
    let dom = context.dom().clone();
    let element_for_effect = element.clone();
    owner.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let mut next = static_values.clone();
            for plan in &properties {
                if let ReactiveBindingTarget::StyleProperty(name) = &plan.target {
                    next.insert(name.to_string(), plan.string_value()?);
                }
            }
            for plan in &sheets {
                for (key, value) in parse_style(&plan.string_value()?) {
                    next.insert(key.to_string(), value.to_string());
                }
            }
            let text = next
                .iter()
                .map(|(key, value)| format!("{key}: {value};"))
                .collect::<Vec<_>>()
                .join(" ");
            dom.set_attribute(AttributeRequest::new(
                &element_for_effect,
                AttributeTarget::Style,
                AttributeValue::text(text),
            ))
            .map_err(Into::into)
        }),
        handler,
    )?;
    let dom_for_cleanup = context.dom().clone();
    let element_for_cleanup = element.clone();
    owner.on_cleanup(
        Box::new(move || {
            let text = static_for_cleanup
                .iter()
                .map(|(key, value)| format!("{key}: {value};"))
                .collect::<Vec<_>>()
                .join(" ");
            dom_for_cleanup
                .set_attribute(AttributeRequest::new(
                    &element_for_cleanup,
                    AttributeTarget::Style,
                    if text.is_empty() {
                        AttributeValue::Removed
                    } else {
                        AttributeValue::text(text)
                    },
                ))
                .map_err(Into::into)
        }),
        handler,
    )
}
