use super::{
    dom::{
        apply_attr_target, apply_bool_target, apply_string_target, cleanup_target, set_classes,
        set_style_property,
    },
    model::{ApplyTarget, Attr},
};
use crate::{kernel::MountContext, lifecycle::MountOwnerToken};
use silex_core::{EffectPhase, ReactiveError, Rx, RxGet, SilexError, SilexResult};
use silex_dom::model::attribute::{AttributeRequest, AttributeTarget, AttributeValue};
use silex_dom::{model::DomElement, runtime::DomContext};
use std::{borrow::Cow, cell::Cell, collections::HashSet, fmt, rc::Rc};
pub(crate) type CustomAttribute<'scope> =
    Rc<dyn Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope>;
type BindingEffect<'scope> = Rc<dyn Fn(&DomContext, &DomElement) -> SilexResult<()> + 'scope>;
type BindingCleanup<'scope> = Rc<dyn Fn(&DomContext, &DomElement) -> SilexResult<()> + 'scope>;
type BindingString<'scope> = Rc<dyn Fn() -> SilexResult<String> + 'scope>;
type BindingBool<'scope> = Rc<dyn Fn() -> SilexResult<bool> + 'scope>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactiveBindingTarget<'scope> {
    Attribute(ApplyTarget),
    ClassToggle(Cow<'scope, str>),
    DynamicClasses,
    StyleProperty(Cow<'scope, str>),
    DynamicStyle,
    Custom,
}

pub struct ReactiveBindingPlan<'scope> {
    pub target: ReactiveBindingTarget<'scope>,
    initial: BindingEffect<'scope>,
    update: BindingEffect<'scope>,
    cleanup: BindingCleanup<'scope>,
    string_value: Option<BindingString<'scope>>,
    bool_value: Option<BindingBool<'scope>>,
}

impl Clone for ReactiveBindingPlan<'_> {
    fn clone(&self) -> Self {
        Self {
            target: self.target.clone(),
            initial: self.initial.clone(),
            update: self.update.clone(),
            cleanup: self.cleanup.clone(),
            string_value: self.string_value.clone(),
            bool_value: self.bool_value.clone(),
        }
    }
}
impl PartialEq for ReactiveBindingPlan<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target
    }
}
impl fmt::Debug for ReactiveBindingPlan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ReactiveBindingPlan")
            .field(&self.target)
            .finish()
    }
}

impl<'scope> ReactiveBindingPlan<'scope> {
    fn effect(
        target: ReactiveBindingTarget<'scope>,
        update: BindingEffect<'scope>,
        cleanup: BindingCleanup<'scope>,
    ) -> Self {
        Self {
            target,
            initial: update.clone(),
            update,
            cleanup,
            string_value: None,
            bool_value: None,
        }
    }

    fn with_string_value(mut self, value: BindingString<'scope>) -> Self {
        self.string_value = Some(value);
        self
    }

    fn with_bool_value(mut self, value: BindingBool<'scope>) -> Self {
        self.bool_value = Some(value);
        self
    }

    pub fn class_toggle(name: Cow<'scope, str>, rx: Rx<'scope, bool>) -> Self {
        let value = Rc::new(move || rx.get());
        let update_value = value.clone();
        let update_name = name.clone();
        let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
            let mut classes = HashSet::new();
            if update_value()? {
                classes.insert(update_name.to_string());
            }
            set_classes(dom, element, &classes)
        });
        let cleanup_name = name.clone();
        let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
            let mut classes = HashSet::new();
            classes.insert(cleanup_name.to_string());
            set_classes(dom, element, &classes)
        });
        Self::effect(ReactiveBindingTarget::ClassToggle(name), update, cleanup)
            .with_bool_value(value)
    }

    pub fn dynamic_classes(rx: Rx<'scope, String>) -> Self {
        let value = Rc::new(move || rx.get());
        let update_value = value.clone();
        let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
            dom.set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Class,
                AttributeValue::text(update_value()?),
            ))
            .map_err(Into::into)
        });
        let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
            dom.set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Class,
                AttributeValue::Removed,
            ))
            .map_err(Into::into)
        });
        Self::effect(ReactiveBindingTarget::DynamicClasses, update, cleanup)
            .with_string_value(value)
    }

    pub fn style_property(name: Cow<'scope, str>, rx: Rx<'scope, String>) -> Self {
        let value = Rc::new(move || rx.get());
        let update_value = value.clone();
        let update_name = name.clone();
        let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
            set_style_property(dom, element, update_name.as_ref(), &update_value()?)
        });
        let cleanup_name = name.clone();
        let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
            set_style_property(dom, element, cleanup_name.as_ref(), "")
        });
        Self::effect(ReactiveBindingTarget::StyleProperty(name), update, cleanup)
            .with_string_value(value)
    }

    pub fn dynamic_style(rx: Rx<'scope, String>) -> Self {
        let value = Rc::new(move || rx.get());
        let update_value = value.clone();
        let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
            dom.set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Style,
                AttributeValue::text(update_value()?),
            ))
            .map_err(Into::into)
        });
        let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
            dom.set_attribute(AttributeRequest::new(
                element,
                AttributeTarget::Style,
                AttributeValue::Removed,
            ))
            .map_err(Into::into)
        });
        Self::effect(ReactiveBindingTarget::DynamicStyle, update, cleanup).with_string_value(value)
    }

    pub(crate) fn string_value(&self) -> SilexResult<String> {
        self.string_value
            .as_ref()
            .ok_or_else(|| SilexError::fatal(ReactiveError::NoSuchNode))
            .and_then(|value| value())
    }

    pub(crate) fn bool_value(&self) -> SilexResult<bool> {
        self.bool_value
            .as_ref()
            .ok_or_else(|| SilexError::fatal(ReactiveError::NoSuchNode))
            .and_then(|value| value())
    }

    pub(crate) fn install(
        self,
        element: &DomElement,
        owner: &MountOwnerToken<'scope>,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let first = Rc::new(Cell::new(true));
        let first_for_effect = first.clone();
        let initial = self.initial;
        let update = self.update;
        let dom = context.dom().clone();
        let dom_for_cleanup = dom.clone();
        let element_for_effect = element.clone();
        let element_for_cleanup = element.clone();
        owner.effect(
            EffectPhase::Normal,
            Box::new(move || {
                if first_for_effect.replace(false) {
                    initial(&dom, &element_for_effect)
                } else {
                    update(&dom, &element_for_effect)
                }
            }),
            context.error_handler(),
        )?;
        owner.on_cleanup(
            Box::new(move || (self.cleanup)(&dom_for_cleanup, &element_for_cleanup)),
            context.error_handler(),
        )
    }
}

pub trait ReactiveBinding<'scope> {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>>
    where
        Self: Sized;
}

pub enum ReactiveBindingContext {
    Value(ApplyTarget),
    Pair {
        key: Cow<'static, str>,
        target: ApplyTarget,
    },
}

fn string_plan<'scope, T>(
    rx: Rx<'scope, T>,
    target: ReactiveBindingTarget<'scope>,
) -> ReactiveBindingPlan<'scope>
where
    T: ToString + Clone + 'scope,
{
    let value = Rc::new(move || rx.get().map(|value| value.to_string()));
    let update_value = value.clone();
    let target_for_update = target.clone();
    let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
        apply_string_target(dom, element, &target_for_update, &update_value()?)
    });
    let target_for_cleanup = target.clone();
    let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
        cleanup_target(dom, element, &target_for_cleanup)
    });
    ReactiveBindingPlan::effect(target, update, cleanup).with_string_value(value)
}

fn bool_plan<'scope>(
    rx: Rx<'scope, bool>,
    target: ReactiveBindingTarget<'scope>,
) -> ReactiveBindingPlan<'scope> {
    let value = Rc::new(move || rx.get());
    let update_value = value.clone();
    let target_for_update = target.clone();
    let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
        apply_bool_target(dom, element, &target_for_update, update_value()?)
    });
    let target_for_cleanup = target.clone();
    let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
        cleanup_target(dom, element, &target_for_cleanup)
    });
    ReactiveBindingPlan::effect(target, update, cleanup).with_bool_value(value)
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
        ApplyTarget::Class => Some(ReactiveBindingTarget::ClassToggle(key)),
        ApplyTarget::Apply => Some(ReactiveBindingTarget::Attribute(ApplyTarget::attr(key))),
        ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
            Some(ReactiveBindingTarget::Attribute(target))
        }
    }
}

fn plan_for_ctx<'scope, T>(
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

impl<'scope> ReactiveBinding<'scope> for String {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        plan_for_ctx(rx, context)
    }
}
impl<'scope, 'a: 'scope> ReactiveBinding<'scope> for &'a str {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        plan_for_ctx(rx, context)
    }
}
impl<'scope, 'a: 'scope> ReactiveBinding<'scope> for Cow<'a, str> {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        plan_for_ctx(rx, context)
    }
}
impl<'scope> ReactiveBinding<'scope> for bool {
    fn binding_plan(
        rx: Rx<'scope, Self>,
        context: ReactiveBindingContext,
    ) -> Option<ReactiveBindingPlan<'scope>> {
        match context {
            ReactiveBindingContext::Value(target) => {
                target_for_value(target).map(|target| bool_plan(rx, target))
            }
            ReactiveBindingContext::Pair { key, target } => match target {
                ApplyTarget::Class => Some(bool_plan(rx, ReactiveBindingTarget::ClassToggle(key))),
                _ => None,
            },
        }
    }
}

macro_rules! impl_reactive_string { ($($ty:ty),*) => { $(impl<'scope> ReactiveBinding<'scope> for $ty { fn binding_plan(rx: Rx<'scope, Self>, context: ReactiveBindingContext) -> Option<ReactiveBindingPlan<'scope>> { plan_for_ctx(rx, context) } })* }; }
impl_reactive_string!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

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
                _ => None,
            },
            ReactiveBindingContext::Pair { key, target } => match target {
                ApplyTarget::Apply => Some(attr_plan(rx, ApplyTarget::attr(key))),
                ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                    Some(attr_plan(rx, target))
                }
                _ => None,
            },
        }
    }
}

fn attr_plan<'scope>(
    rx: Rx<'scope, Attr<'scope>>,
    target: ApplyTarget,
) -> ReactiveBindingPlan<'scope> {
    let target_for_update = target.clone();
    let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
        let value = rx.get()?;
        apply_attr_target(dom, element, &target_for_update, &value)
    });
    let target_for_cleanup = target.clone();
    let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
        cleanup_target(
            dom,
            element,
            &ReactiveBindingTarget::Attribute(target_for_cleanup.clone()),
        )
    });
    ReactiveBindingPlan::effect(ReactiveBindingTarget::Attribute(target), update, cleanup)
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
            ReactiveBindingContext::Value(target) => target_for_value(target).map(|target| {
                let value =
                    Rc::new(move || Ok(rx.get()?.map(|item| item.to_string()).unwrap_or_default()));
                let update_value = value.clone();
                let target_for_update = target.clone();
                let update = Rc::new(move |dom: &DomContext, element: &DomElement| {
                    apply_string_target(dom, element, &target_for_update, &update_value()?)
                });
                let target_for_cleanup = target.clone();
                let cleanup = Rc::new(move |dom: &DomContext, element: &DomElement| {
                    cleanup_target(dom, element, &target_for_cleanup)
                });
                ReactiveBindingPlan::effect(target, update, cleanup).with_string_value(value)
            }),
            ReactiveBindingContext::Pair { .. } => None,
        }
    }
}
