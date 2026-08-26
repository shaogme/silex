use super::{
    binding::ReactiveBindingTarget,
    model::{ApplyTarget, Attr, KnownProp},
};
use silex_core::SilexResult;
use silex_dom::{
    model::{
        DomElement,
        attribute::{
            AttributeRequest, AttributeTarget, AttributeValue, PropertyRequest, PropertyValue,
        },
    },
    runtime::DomContext,
};
use std::collections::HashSet;
pub(crate) fn attr_value(value: &Attr<'_>) -> AttributeValue {
    match value {
        Attr::Removed => AttributeValue::Removed,
        Attr::Empty => AttributeValue::Empty,
        Attr::String(value) => AttributeValue::text(value.to_string()),
    }
}
pub(crate) fn property_value(target: &ApplyTarget, value: &Attr<'_>) -> PropertyValue {
    match value {
        Attr::Removed => PropertyValue::Removed,
        Attr::Empty => {
            if matches!(target, ApplyTarget::Known(KnownProp::Value)) {
                PropertyValue::String(String::new())
            } else {
                PropertyValue::Bool(true)
            }
        }
        Attr::String(value) => PropertyValue::String(value.to_string()),
    }
}

pub(crate) fn apply_attr_target(
    dom: &DomContext,
    element: &DomElement,
    target: &ApplyTarget,
    value: &Attr<'_>,
) -> SilexResult<()> {
    match target {
        ApplyTarget::Prop(_)
        | ApplyTarget::Known(KnownProp::Value)
        | ApplyTarget::Known(KnownProp::Checked)
        | ApplyTarget::Known(KnownProp::Disabled)
        | ApplyTarget::Known(KnownProp::ReadOnly)
        | ApplyTarget::Known(KnownProp::Required) => dom
            .set_property(PropertyRequest::new(
                element,
                target.attr_name(),
                property_value(target, value),
            ))
            .map_err(Into::into),
        ApplyTarget::Attr(name) => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::named(name.clone()),
                attr_value(value),
            ))
            .map_err(Into::into),
        ApplyTarget::Class => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Class,
                attr_value(value),
            ))
            .map_err(Into::into),
        ApplyTarget::Style => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Style,
                attr_value(value),
            ))
            .map_err(Into::into),
        ApplyTarget::Apply => Ok(()),
    }
}

pub(crate) fn apply_string_target(
    dom: &DomContext,
    element: &DomElement,
    target: &ReactiveBindingTarget<'_>,
    value: &str,
) -> SilexResult<()> {
    match target {
        ReactiveBindingTarget::Attribute(target) => {
            apply_attr_target(dom, element, target, &Attr::from(value.to_string()))
        }
        ReactiveBindingTarget::DynamicClasses => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Class,
                AttributeValue::text(value),
            ))
            .map_err(Into::into),
        ReactiveBindingTarget::DynamicStyle => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Style,
                AttributeValue::text(value),
            ))
            .map_err(Into::into),
        ReactiveBindingTarget::StyleProperty(name) => set_style_property(dom, element, name, value),
        ReactiveBindingTarget::ClassToggle(_) | ReactiveBindingTarget::Custom => Ok(()),
    }
}

pub(crate) fn apply_bool_target(
    dom: &DomContext,
    element: &DomElement,
    target: &ReactiveBindingTarget<'_>,
    value: bool,
) -> SilexResult<()> {
    match target {
        ReactiveBindingTarget::Attribute(target) => {
            apply_attr_target(dom, element, target, &Attr::from(value))
        }
        ReactiveBindingTarget::ClassToggle(name) => {
            let mut classes = HashSet::new();
            if value {
                classes.insert(name.to_string());
            }
            set_classes(dom, element, &classes)
        }
        _ => Ok(()),
    }
}

pub(crate) fn cleanup_target(
    dom: &DomContext,
    element: &DomElement,
    target: &ReactiveBindingTarget<'_>,
) -> SilexResult<()> {
    match target {
        ReactiveBindingTarget::Attribute(target) => match target {
            ApplyTarget::Prop(_) | ApplyTarget::Known(_) => dom
                .set_property(PropertyRequest::new(
                    element,
                    target.attr_name(),
                    PropertyValue::Removed,
                ))
                .map_err(Into::into),
            ApplyTarget::Attr(name) => dom
                .set_attribute(AttributeRequest::new(
                    element,
                    AttributeTarget::named(name.clone()),
                    AttributeValue::Removed,
                ))
                .map_err(Into::into),
            ApplyTarget::Class => dom
                .set_attribute(AttributeRequest::new(
                    element,
                    AttributeTarget::Class,
                    AttributeValue::Removed,
                ))
                .map_err(Into::into),
            ApplyTarget::Style => dom
                .set_attribute(AttributeRequest::new(
                    element,
                    AttributeTarget::Style,
                    AttributeValue::Removed,
                ))
                .map_err(Into::into),
            ApplyTarget::Apply => Ok(()),
        },
        ReactiveBindingTarget::DynamicClasses => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Class,
                AttributeValue::Removed,
            ))
            .map_err(Into::into),
        ReactiveBindingTarget::DynamicStyle => dom
            .set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Style,
                AttributeValue::Removed,
            ))
            .map_err(Into::into),
        ReactiveBindingTarget::StyleProperty(name) => set_style_property(dom, element, name, ""),
        ReactiveBindingTarget::ClassToggle(name) => {
            let mut classes = HashSet::new();
            classes.insert(name.to_string());
            set_classes(dom, element, &classes)
        }
        ReactiveBindingTarget::Custom => Ok(()),
    }
}

pub(crate) fn set_classes(
    dom: &DomContext,
    element: &DomElement,
    classes: &HashSet<String>,
) -> SilexResult<()> {
    let mut values = classes.iter().cloned().collect::<Vec<_>>();
    values.sort();
    dom.set_attribute(AttributeRequest::new(
        element,
        AttributeTarget::Class,
        if values.is_empty() {
            AttributeValue::Removed
        } else {
            AttributeValue::text(values.join(" "))
        },
    ))
    .map_err(Into::into)
}

/// 用 backend-neutral request 替换元素的 class 属性。
pub fn set_class_value(dom: &DomContext, element: &DomElement, value: &str) -> SilexResult<()> {
    dom.set_attribute(AttributeRequest::new(
        element,
        AttributeTarget::Class,
        if value.is_empty() {
            AttributeValue::Removed
        } else {
            AttributeValue::text(value)
        },
    ))
    .map_err(Into::into)
}

/// 增删一组 class token，同时保留其它属性来源写入的 class。
pub fn update_class_tokens(
    dom: &DomContext,
    element: &DomElement,
    add: impl IntoIterator<Item = String>,
    remove: impl IntoIterator<Item = String>,
) -> SilexResult<()> {
    dom.set_attribute(AttributeRequest::new(
        element,
        AttributeTarget::Class,
        AttributeValue::ClassTokens {
            add: add.into_iter().collect(),
            remove: remove.into_iter().collect(),
        },
    ))
    .map_err(Into::into)
}

pub(crate) fn set_style_property(
    dom: &DomContext,
    element: &DomElement,
    name: &str,
    value: &str,
) -> SilexResult<()> {
    dom.set_style_property(
        element,
        name,
        if value.is_empty() { None } else { Some(value) },
    )
    .map_err(Into::into)
}

pub(crate) fn parse_style(value: &str) -> impl Iterator<Item = (&str, &str)> {
    value.split(';').filter_map(|rule| {
        let rule = rule.trim();
        if rule.is_empty() {
            None
        } else {
            rule.split_once(':')
                .map(|(name, value)| (name.trim(), value.trim()))
        }
    })
}
