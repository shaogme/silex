use std::{borrow::Cow, rc::Rc};

use silex_core::{Rx, RxValueKind, SilexError, SilexErrorKind, SilexResult};
use wasm_bindgen::JsValue;
use web_sys::Element as WebElem;

use super::foundation::{
    ApplyTarget, ApplyToDom, ReactiveBinding, ReactiveBindingContext, ReactiveBindingPlan,
    ReactiveBindingTarget, apply_immediate_string,
};
use crate::attribute::op::{
    Attr, AttrOp, apply_attr_with_target_internal, apply_immediate_bool_internal, get_style_decl,
};
use crate::view::{MountErrorHandler, MountOwnerToken};

fn cleanup_target(el: &WebElem, target: &ApplyTarget) -> SilexResult<()> {
    match target {
        ApplyTarget::Attr(name) => el.remove_attribute(name).map_err(SilexError::fatal),
        ApplyTarget::Prop(name) => {
            js_sys::Reflect::set(el, &JsValue::from_str(name), &JsValue::UNDEFINED)
                .map(|_| ())
                .map_err(SilexError::fatal)
        }
        ApplyTarget::Known(prop) => apply_attr_with_target_internal(
            el,
            prop.name(),
            ApplyTarget::Known(*prop),
            &Attr::Removed,
        ),
        ApplyTarget::Class | ApplyTarget::Style | ApplyTarget::Apply => Ok(()),
    }
}

fn string_value<'scope, T>(rx: Rx<'scope, T>) -> Rc<dyn Fn() -> SilexResult<String> + 'scope>
where
    T: ToString + Clone + 'scope,
{
    Rc::new(move || rx.get().map(|value| value.to_string()))
}

fn string_plan<'scope, T>(
    rx: Rx<'scope, T>,
    target: ReactiveBindingTarget<'scope>,
) -> ReactiveBindingPlan<'scope>
where
    T: ToString + Clone + 'scope,
{
    let value = string_value(rx);
    let value_for_update = value.clone();
    let target_for_update = target.clone();
    let update = Rc::new(move |el: &WebElem| {
        let value = value_for_update()?;
        match &target_for_update {
            ReactiveBindingTarget::Attribute(target) => match target {
                ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                    apply_attr_with_target_internal(
                        el,
                        target.attr_name(),
                        target.clone(),
                        &Attr::from(value),
                    )
                }
                ApplyTarget::Class => el.set_attribute("class", &value).map_err(SilexError::fatal),
                ApplyTarget::Style => {
                    if let Some(style) = get_style_decl(el) {
                        style.set_css_text(&value);
                        Ok(())
                    } else {
                        Err(SilexError::fatal(SilexErrorKind::Dom(
                            "element does not expose a style declaration".to_string(),
                        )))
                    }
                }
                ApplyTarget::Apply => Ok(()),
            },
            ReactiveBindingTarget::DynamicClasses => {
                el.set_attribute("class", &value).map_err(SilexError::fatal)
            }
            ReactiveBindingTarget::DynamicStyle => {
                if let Some(style) = get_style_decl(el) {
                    style.set_css_text(&value);
                    Ok(())
                } else {
                    Err(SilexError::fatal(SilexErrorKind::Dom(
                        "element does not expose a style declaration".to_string(),
                    )))
                }
            }
            ReactiveBindingTarget::StyleProperty(name) => {
                let style = get_style_decl(el).ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Dom(
                        "element does not expose a style declaration".to_string(),
                    ))
                })?;
                style.set_property(name, &value).map_err(SilexError::fatal)
            }
            ReactiveBindingTarget::ClassToggle(_) | ReactiveBindingTarget::Custom => Ok(()),
        }
    });

    let target_for_cleanup = target.clone();
    let cleanup = Rc::new(move |el: &WebElem| match &target_for_cleanup {
        ReactiveBindingTarget::Attribute(target) => cleanup_target(el, target),
        ReactiveBindingTarget::DynamicClasses => {
            el.remove_attribute("class").map_err(SilexError::fatal)
        }
        ReactiveBindingTarget::DynamicStyle => get_style_decl(el)
            .map(|style| style.set_css_text(""))
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom(
                    "element does not expose a style declaration".to_string(),
                ))
            }),
        ReactiveBindingTarget::StyleProperty(name) => get_style_decl(el)
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom(
                    "element does not expose a style declaration".to_string(),
                ))
            })?
            .remove_property(name)
            .map(|_| ())
            .map_err(SilexError::fatal),
        ReactiveBindingTarget::ClassToggle(name) => {
            el.class_list().remove_1(name).map_err(SilexError::fatal)
        }
        ReactiveBindingTarget::Custom => Ok(()),
    });

    ReactiveBindingPlan::effect(target, update, cleanup).with_string_value(value)
}

fn bool_value<'scope>(rx: Rx<'scope, bool>) -> Rc<dyn Fn() -> SilexResult<bool> + 'scope> {
    Rc::new(move || rx.get())
}

fn bool_plan<'scope>(
    rx: Rx<'scope, bool>,
    target: ReactiveBindingTarget<'scope>,
) -> ReactiveBindingPlan<'scope> {
    let value = bool_value(rx);
    let value_for_update = value.clone();
    let target_for_update = target.clone();
    let update = Rc::new(move |el: &WebElem| {
        let value = value_for_update()?;
        match &target_for_update {
            ReactiveBindingTarget::Attribute(target) => {
                apply_immediate_bool_internal(el, target.attr_name(), value, true)
            }
            ReactiveBindingTarget::ClassToggle(name) => {
                if value {
                    el.class_list().add_1(name).map_err(SilexError::fatal)
                } else {
                    el.class_list().remove_1(name).map_err(SilexError::fatal)
                }
            }
            ReactiveBindingTarget::DynamicClasses => el
                .class_list()
                .toggle_with_force("active", value)
                .map(|_| ())
                .map_err(SilexError::fatal),
            ReactiveBindingTarget::DynamicStyle | ReactiveBindingTarget::Custom => Ok(()),
            ReactiveBindingTarget::StyleProperty(_) => Ok(()),
        }
    });

    let target_for_cleanup = target.clone();
    let cleanup = Rc::new(move |el: &WebElem| match &target_for_cleanup {
        ReactiveBindingTarget::Attribute(target) => cleanup_target(el, target),
        ReactiveBindingTarget::ClassToggle(name) => {
            el.class_list().remove_1(name).map_err(SilexError::fatal)
        }
        ReactiveBindingTarget::DynamicClasses => el
            .class_list()
            .remove_1("active")
            .map_err(SilexError::fatal),
        _ => Ok(()),
    });

    ReactiveBindingPlan::effect(target, update, cleanup).with_bool_value(value)
}

fn attr_plan<'scope>(
    rx: Rx<'scope, Attr<'scope>>,
    target: ApplyTarget,
) -> ReactiveBindingPlan<'scope> {
    let target_for_update = target.clone();
    let update = Rc::new(move |el: &WebElem| {
        let value = rx.get()?;
        apply_attr_with_target_internal(
            el,
            target_for_update.attr_name(),
            target_for_update.clone(),
            &value,
        )
    });
    let target_for_cleanup = target.clone();
    let cleanup = Rc::new(move |el: &WebElem| cleanup_target(el, &target_for_cleanup));
    ReactiveBindingPlan::effect(ReactiveBindingTarget::Attribute(target), update, cleanup)
}

fn target_for_value(target: ApplyTarget) -> Option<ReactiveBindingTarget<'static>> {
    match target {
        ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
            Some(ReactiveBindingTarget::Attribute(target))
        }
        ApplyTarget::Class => Some(ReactiveBindingTarget::DynamicClasses),
        ApplyTarget::Style => Some(ReactiveBindingTarget::DynamicStyle),
        ApplyTarget::Apply => None,
    }
}

fn target_for_pair(
    key: Cow<'static, str>,
    target: ApplyTarget,
) -> Option<ReactiveBindingTarget<'static>> {
    match target {
        ApplyTarget::Style => Some(ReactiveBindingTarget::StyleProperty(key)),
        ApplyTarget::Class => Some(ReactiveBindingTarget::DynamicClasses),
        ApplyTarget::Apply => Some(ReactiveBindingTarget::Attribute(ApplyTarget::attr(key))),
        ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
            Some(ReactiveBindingTarget::Attribute(target))
        }
    }
}

fn plan_for_context<'scope, T>(
    rx: Rx<'scope, T>,
    context: ReactiveBindingContext,
) -> Option<ReactiveBindingPlan<'scope>>
where
    T: ToString + Clone + 'scope,
{
    match context {
        ReactiveBindingContext::Value(target) => {
            target_for_value(target).map(|target| string_plan(rx, target))
        }
        ReactiveBindingContext::Pair { key, target } => {
            target_for_pair(key, target).map(|target| string_plan(rx, target))
        }
    }
}

fn bool_plan_for_context<'scope>(
    rx: Rx<'scope, bool>,
    context: ReactiveBindingContext,
) -> Option<ReactiveBindingPlan<'scope>> {
    match context {
        ReactiveBindingContext::Value(target) => match target {
            ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                Some(bool_plan(rx, ReactiveBindingTarget::Attribute(target)))
            }
            ApplyTarget::Class => Some(bool_plan(rx, ReactiveBindingTarget::DynamicClasses)),
            ApplyTarget::Style | ApplyTarget::Apply => None,
        },
        ReactiveBindingContext::Pair { key, target } => match target {
            ApplyTarget::Class => Some(bool_plan(rx, ReactiveBindingTarget::ClassToggle(key))),
            ApplyTarget::Apply
            | ApplyTarget::Attr(_)
            | ApplyTarget::Prop(_)
            | ApplyTarget::Known(_)
            | ApplyTarget::Style => None,
        },
    }
}

pub(crate) fn apply_rx_internal<'scope, T>(
    rx: Rx<'scope, T>,
    el: &WebElem,
    target: ApplyTarget,
    owner: &MountOwnerToken<'scope>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<()>
where
    T: ReactiveBinding<'scope> + 'scope,
{
    if let Some(plan) = T::binding_plan(rx, ReactiveBindingContext::Value(target)) {
        plan.install(el, owner, error_handler)?;
    }
    Ok(())
}

impl<'scope, T> ApplyToDom<'scope> for Rx<'scope, T, RxValueKind>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &MountOwnerToken<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        apply_rx_internal(*self, el, target, owner, error_handler)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        T::binding_plan(self, ReactiveBindingContext::Value(target))
            .map(AttrOp::Reactive)
            .unwrap_or(AttrOp::Noop)
    }
}

impl<'scope> ReactiveBinding<'scope> for String {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        plan_for_context(rx, context)
    }
}

macro_rules! impl_reactive_binding_string_like {
    ($($ty:ty),*) => {
        $(
            impl<'scope, 'a: 'scope> ReactiveBinding<'scope> for $ty {
                fn binding_plan(
                    rx: Rx<'scope, Self>,
                    context: ReactiveBindingContext,
                ) -> Option<ReactiveBindingPlan<'scope>> {
                    plan_for_context(rx, context)
                }
            }
        )*
    };
}

impl_reactive_binding_string_like!(&'a str, Cow<'a, str>, &'a String);

macro_rules! impl_reactive_binding_primitive {
    ($($ty:ty),*) => {
        $(
            impl<'scope> ReactiveBinding<'scope> for $ty {
                fn binding_plan(
                    rx: Rx<'scope, Self>,
                    context: ReactiveBindingContext,
                ) -> Option<ReactiveBindingPlan<'scope>> {
                    match context {
                        ReactiveBindingContext::Value(target) => {
                            target_for_value(target).map(|target| string_plan(rx, target))
                        }
                        ReactiveBindingContext::Pair { .. } => None,
                    }
                }
            }
        )*
    };
}

impl_reactive_binding_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl<'scope> ReactiveBinding<'scope> for bool {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        bool_plan_for_context(rx, context)
    }
}

impl<'scope> ReactiveBinding<'scope> for Attr<'scope> {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        match context {
            ReactiveBindingContext::Value(target) => match target {
                ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                    Some(attr_plan(rx, target))
                }
                ApplyTarget::Class | ApplyTarget::Style | ApplyTarget::Apply => None,
            },
            ReactiveBindingContext::Pair { key, target } => match target {
                ApplyTarget::Apply => Some(attr_plan(rx, ApplyTarget::attr(key))),
                ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                    Some(attr_plan(rx, target))
                }
                ApplyTarget::Class | ApplyTarget::Style => None,
            },
        }
    }
}

impl<'scope, T> ReactiveBinding<'scope> for Option<T>
where
    T: ToString + Clone + 'scope,
{
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        match context {
            ReactiveBindingContext::Value(target) => {
                let value = Rc::new(move || {
                    Ok(rx.get()?.map(|value| value.to_string()).unwrap_or_default())
                });
                target_for_value(target).map(|target| {
                    let value_for_update = value.clone();
                    let target_for_update = target.clone();
                    let update = Rc::new(move |el: &WebElem| {
                        let value = value_for_update()?;
                        match &target_for_update {
                            ReactiveBindingTarget::Attribute(target) => {
                                apply_immediate_string(el, target, &value)
                            }
                            ReactiveBindingTarget::DynamicClasses => {
                                el.set_attribute("class", &value).map_err(SilexError::fatal)
                            }
                            ReactiveBindingTarget::DynamicStyle => {
                                if let Some(style) = get_style_decl(el) {
                                    style.set_css_text(&value);
                                    Ok(())
                                } else {
                                    Err(SilexError::fatal(SilexErrorKind::Dom(
                                        "element does not expose a style declaration".to_string(),
                                    )))
                                }
                            }
                            _ => Ok(()),
                        }
                    });
                    let target_for_cleanup = target.clone();
                    let cleanup = Rc::new(move |el: &WebElem| match &target_for_cleanup {
                        ReactiveBindingTarget::Attribute(target) => cleanup_target(el, target),
                        ReactiveBindingTarget::DynamicClasses => {
                            el.remove_attribute("class").map_err(SilexError::fatal)
                        }
                        ReactiveBindingTarget::DynamicStyle => get_style_decl(el)
                            .map(|style| style.set_css_text(""))
                            .ok_or_else(|| {
                                SilexError::fatal(SilexErrorKind::Dom(
                                    "element does not expose a style declaration".to_string(),
                                ))
                            }),
                        _ => Ok(()),
                    });
                    ReactiveBindingPlan::effect(target, update, cleanup).with_string_value(value)
                })
            }
            ReactiveBindingContext::Pair { .. } => None,
        }
    }
}
