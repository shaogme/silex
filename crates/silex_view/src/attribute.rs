use crate::{
    context::MountContext,
    contract::Prop,
    event::{DomEvent, EventDescriptor, EventHandler},
    owner::{MountErrorHandler, MountOwnerToken},
};
use silex_core::{EffectPhase, ReactiveError, Rx, RxGet, RxWrite, SilexError, SilexResult};
use silex_dom::attribute::{
    AttributeRequest, AttributeTarget, AttributeValue, PropertyRequest, PropertyValue,
};
use silex_dom::node_ref::NodeRef;
use silex_dom::{DomContext, DomElement};
use std::{
    borrow::Cow,
    cell::Cell,
    collections::{BTreeMap, HashSet},
    rc::Rc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyTarget {
    Attr(Cow<'static, str>),
    Prop(Cow<'static, str>),
    Known(KnownProp),
    Class,
    Style,
    Apply,
}

impl ApplyTarget {
    pub fn attr(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        match name.as_ref() {
            "class" => Self::Class,
            "style" => Self::Style,
            _ => KnownProp::parse(name.as_ref()).map_or(Self::Attr(name), Self::Known),
        }
    }

    pub fn prop(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        match name.as_ref() {
            "class" => Self::Class,
            "style" => Self::Style,
            _ => KnownProp::parse(name.as_ref()).map_or(Self::Prop(name), Self::Known),
        }
    }

    pub fn name(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::Attr(name) | Self::Prop(name) => Some(name.clone()),
            Self::Known(prop) => Some(Cow::Borrowed(prop.name())),
            Self::Class => Some(Cow::Borrowed("class")),
            Self::Style => Some(Cow::Borrowed("style")),
            Self::Apply => None,
        }
    }

    pub fn attr_name(&self) -> &str {
        match self {
            Self::Attr(name) | Self::Prop(name) => name,
            Self::Known(prop) => prop.name(),
            Self::Class => "class",
            Self::Style => "style",
            Self::Apply => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownProp {
    Value,
    Checked,
    Disabled,
    ReadOnly,
    Required,
}

impl KnownProp {
    pub fn name(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Checked => "checked",
            Self::Disabled => "disabled",
            Self::ReadOnly => "readOnly",
            Self::Required => "required",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "value" => Some(Self::Value),
            "checked" => Some(Self::Checked),
            "disabled" => Some(Self::Disabled),
            "readOnly" | "readonly" => Some(Self::ReadOnly),
            "required" => Some(Self::Required),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attr<'scope> {
    Removed,
    Empty,
    String(Cow<'scope, str>),
}

impl From<bool> for Attr<'_> {
    fn from(value: bool) -> Self {
        if value { Self::Empty } else { Self::Removed }
    }
}
impl<'a> From<&'a str> for Attr<'a> {
    fn from(value: &'a str) -> Self {
        if value.is_empty() {
            Self::Empty
        } else {
            Self::String(Cow::Borrowed(value))
        }
    }
}
impl From<String> for Attr<'_> {
    fn from(value: String) -> Self {
        if value.is_empty() {
            Self::Empty
        } else {
            Self::String(Cow::Owned(value))
        }
    }
}
impl<'scope> From<Cow<'scope, str>> for Attr<'scope> {
    fn from(value: Cow<'scope, str>) -> Self {
        if value.is_empty() {
            Self::Empty
        } else {
            Self::String(value)
        }
    }
}
impl<'scope, T: Into<Attr<'scope>>> From<Option<T>> for Attr<'scope> {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Removed, Into::into)
    }
}

#[derive(Clone)]
pub enum AttrData<'scope> {
    StaticAttr(Attr<'scope>),
    ReactiveAttr(Rx<'scope, Attr<'scope>>),
    ReactiveString(Rx<'scope, String>),
    ReactiveBool(Rx<'scope, bool>),
    ReactiveOptionString(Rx<'scope, Option<String>>),
}

impl std::fmt::Debug for AttrData<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaticAttr(value) => formatter.debug_tuple("StaticAttr").field(value).finish(),
            Self::ReactiveAttr(_) => formatter.write_str("ReactiveAttr(Rx)"),
            Self::ReactiveString(_) => formatter.write_str("ReactiveString(Rx)"),
            Self::ReactiveBool(_) => formatter.write_str("ReactiveBool(Rx)"),
            Self::ReactiveOptionString(_) => formatter.write_str("ReactiveOptionString(Rx)"),
        }
    }
}

impl PartialEq for AttrData<'_> {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (Self::StaticAttr(left), Self::StaticAttr(right)) if left == right)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttrUpdate<'scope> {
    pub target: ApplyTarget,
    pub data: AttrData<'scope>,
}

#[derive(Clone)]
pub struct CombinedClasses<'scope> {
    pub statics: Vec<Cow<'scope, str>>,
    pub toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
    pub reactives: Vec<ReactiveBindingPlan<'scope>>,
}

impl PartialEq for CombinedClasses<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.toggles == other.toggles
            && self.reactives == other.reactives
    }
}
impl std::fmt::Debug for CombinedClasses<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CombinedClasses")
            .field("statics", &self.statics)
            .field("toggles", &self.toggles.len())
            .field("reactives", &self.reactives.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct CombinedStyles<'scope> {
    pub statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
    pub properties: Vec<ReactiveBindingPlan<'scope>>,
    pub sheets: Vec<ReactiveBindingPlan<'scope>>,
}

impl PartialEq for CombinedStyles<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.properties == other.properties
            && self.sheets == other.sheets
    }
}
impl std::fmt::Debug for CombinedStyles<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CombinedStyles")
            .field("statics", &self.statics)
            .field("properties", &self.properties.len())
            .field("sheets", &self.sheets.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrPhase {
    Staging,
    Commit,
}

type CustomAttribute<'scope> =
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
impl std::fmt::Debug for ReactiveBindingPlan<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

    fn string_value(&self) -> SilexResult<String> {
        self.string_value
            .as_ref()
            .ok_or_else(|| SilexError::fatal(ReactiveError::NoSuchNode))
            .and_then(|value| value())
    }

    fn bool_value(&self) -> SilexResult<bool> {
        self.bool_value
            .as_ref()
            .ok_or_else(|| SilexError::fatal(ReactiveError::NoSuchNode))
            .and_then(|value| value())
    }

    fn install(
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

fn attr_value(value: &Attr<'_>) -> AttributeValue {
    match value {
        Attr::Removed => AttributeValue::Removed,
        Attr::Empty => AttributeValue::Empty,
        Attr::String(value) => AttributeValue::text(value.to_string()),
    }
}
fn property_value(target: &ApplyTarget, value: &Attr<'_>) -> PropertyValue {
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

fn apply_attr_target(
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

fn apply_string_target(
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

fn apply_bool_target(
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

fn cleanup_target(
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

fn set_classes(
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

fn set_style_property(
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

fn parse_style(value: &str) -> impl Iterator<Item = (&str, &str)> {
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

#[derive(Clone)]
pub enum AttrOp<'scope> {
    Update(AttrUpdate<'scope>),
    CombinedClasses(CombinedClasses<'scope>),
    CombinedStyles(CombinedStyles<'scope>),
    Reactive(ReactiveBindingPlan<'scope>),
    Sequence(Vec<AttrOp<'scope>>),
    Custom {
        phase: AttrPhase,
        callback: CustomAttribute<'scope>,
    },
    Noop,
}

impl std::fmt::Debug for AttrOp<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Update(value) => formatter.debug_tuple("Update").field(value).finish(),
            Self::CombinedClasses(value) => formatter
                .debug_tuple("CombinedClasses")
                .field(value)
                .finish(),
            Self::CombinedStyles(value) => formatter
                .debug_tuple("CombinedStyles")
                .field(value)
                .finish(),
            Self::Reactive(value) => formatter.debug_tuple("Reactive").field(value).finish(),
            Self::Sequence(value) => formatter.debug_tuple("Sequence").field(value).finish(),
            Self::Custom { .. } => formatter.write_str("Custom(Rc<Fn>)"),
            Self::Noop => formatter.write_str("Noop"),
        }
    }
}
impl PartialEq for AttrOp<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Update(left), Self::Update(right)) => left == right,
            (Self::CombinedClasses(left), Self::CombinedClasses(right)) => left == right,
            (Self::CombinedStyles(left), Self::CombinedStyles(right)) => left == right,
            (Self::Reactive(left), Self::Reactive(right)) => left == right,
            (Self::Sequence(left), Self::Sequence(right)) => left == right,
            (
                Self::Custom {
                    phase: left_phase,
                    callback: left,
                },
                Self::Custom {
                    phase: right_phase,
                    callback: right,
                },
            ) => left_phase == right_phase && Rc::ptr_eq(left, right),
            (Self::Noop, Self::Noop) => true,
            _ => false,
        }
    }
}

impl<'scope> AttrOp<'scope> {
    pub fn build<V>(value: V, target: ApplyTarget) -> Self
    where
        V: ApplyToDom<'scope> + 'scope,
    {
        value.into_op(target)
    }

    pub fn static_class(value: Cow<'scope, str>) -> Self {
        Self::CombinedClasses(CombinedClasses {
            statics: vec![value],
            toggles: Vec::new(),
            reactives: Vec::new(),
        })
    }

    pub fn static_classes(values: Vec<Cow<'scope, str>>) -> Self {
        Self::CombinedClasses(CombinedClasses {
            statics: values,
            toggles: Vec::new(),
            reactives: Vec::new(),
        })
    }

    pub fn class_toggle(name: Cow<'scope, str>, rx: Rx<'scope, bool>) -> Self {
        Self::Reactive(ReactiveBindingPlan::class_toggle(name, rx))
    }

    pub fn reactive_classes(rx: Rx<'scope, String>) -> Self {
        Self::Reactive(ReactiveBindingPlan::dynamic_classes(rx))
    }

    pub fn static_styles(values: Vec<(Cow<'scope, str>, Cow<'scope, str>)>) -> Self {
        Self::CombinedStyles(CombinedStyles {
            statics: values,
            properties: Vec::new(),
            sheets: Vec::new(),
        })
    }

    pub fn style_property(name: Cow<'scope, str>, rx: Rx<'scope, String>) -> Self {
        Self::Reactive(ReactiveBindingPlan::style_property(name, rx))
    }

    pub fn reactive_stylesheet(rx: Rx<'scope, String>) -> Self {
        Self::Reactive(ReactiveBindingPlan::dynamic_style(rx))
    }

    pub fn custom(
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::Custom {
            phase: AttrPhase::Staging,
            callback: Rc::new(callback),
        }
    }

    pub fn custom_phase(
        phase: AttrPhase,
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::Custom {
            phase,
            callback: Rc::new(callback),
        }
    }

    pub fn new_scoped(
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::custom(callback)
    }

    pub fn on_commit(
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::custom_phase(AttrPhase::Commit, callback)
    }

    pub fn apply(self, element: &DomElement, context: &MountContext<'scope>) -> SilexResult<()> {
        let owner = context.owner();
        let handler = context.error_handler();
        match self {
            Self::Update(AttrUpdate { target, data }) => {
                apply_update(element, target, data, &owner, context, handler)
            }
            Self::CombinedClasses(value) => apply_classes(element, value, &owner, context, handler),
            Self::CombinedStyles(value) => apply_styles(element, value, &owner, context, handler),
            Self::Reactive(plan) => plan.install(element, &owner, context),
            Self::Sequence(values) => {
                for value in values {
                    value.apply(element, context)?;
                }
                Ok(())
            }
            Self::Custom {
                phase: AttrPhase::Staging,
                callback,
            } => owner.with_runtime(|| callback(element, context))?,
            Self::Custom {
                phase: AttrPhase::Commit,
                callback,
            } => {
                let element = element.clone();
                let commit_context = context.clone();
                context.on_commit(move || callback(&element, &commit_context))?;
                Ok(())
            }
            Self::Noop => Ok(()),
        }
    }
}

fn apply_update<'scope>(
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

fn apply_classes<'scope>(
    element: &DomElement,
    value: CombinedClasses<'scope>,
    owner: &MountOwnerToken<'scope>,
    context: &MountContext<'scope>,
    handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let statics = value
        .statics
        .iter()
        .flat_map(|item| item.split_whitespace().map(ToOwned::to_owned))
        .collect::<HashSet<_>>();
    let static_for_effect = statics.clone();
    let element_for_effect = element.clone();
    let dom = context.dom().clone();
    let toggles = value.toggles;
    let reactives = value.reactives;
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

fn apply_styles<'scope>(
    element: &DomElement,
    value: CombinedStyles<'scope>,
    owner: &MountOwnerToken<'scope>,
    context: &MountContext<'scope>,
    handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let static_values = value
        .statics
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
    let static_for_cleanup = static_values.clone();
    let dom = context.dom().clone();
    let element_for_effect = element.clone();
    let properties = value.properties;
    let sheets = value.sheets;
    owner.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let mut next = static_values.clone();
            for plan in &properties {
                if let crate::attribute::ReactiveBindingTarget::StyleProperty(name) = &plan.target {
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

pub trait ApplyToDom<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()>;
    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope>
    where
        Self: Sized + 'scope,
    {
        AttrOp::custom(move |element, context| self.apply(element, target.clone(), context))
    }
}

impl<'scope> ApplyToDom<'scope> for AttrOp<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        _target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        self.clone().apply(element, context)
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        self
    }
}
impl<'scope> ApplyToDom<'scope> for AttributeGroup<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        _target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        for op in &self.0 {
            op.clone().apply(element, context)?;
        }
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        if self.0.is_empty() {
            AttrOp::Noop
        } else {
            AttrOp::Sequence(self.0)
        }
    }
}
impl<'scope, 'a: 'scope> ApplyToDom<'scope> for &'a str {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, &Attr::from(*self))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match target {
            ApplyTarget::Class => {
                AttrOp::static_classes(self.split_whitespace().map(Cow::Borrowed).collect())
            }
            ApplyTarget::Style => AttrOp::static_styles(
                parse_style(self)
                    .map(|(key, value)| (Cow::Owned(key.into()), Cow::Owned(value.into())))
                    .collect(),
            ),
            target => AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::StaticAttr(Attr::from(self)),
            }),
        }
    }
}
impl<'scope> ApplyToDom<'scope> for String {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, &Attr::from(self.clone()))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Update(AttrUpdate {
            target,
            data: AttrData::StaticAttr(Attr::from(self)),
        })
    }
}
impl<'scope> ApplyToDom<'scope> for &String {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(
            context.dom(),
            element,
            &target,
            &Attr::from((*self).clone()),
        )
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        self.to_string().into_op(target)
    }
}
impl<'scope, 'a: 'scope> ApplyToDom<'scope> for Cow<'a, str> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(
            context.dom(),
            element,
            &target,
            &Attr::from(self.to_string()),
        )
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match self {
            Cow::Borrowed(value) => value.into_op(target),
            Cow::Owned(value) => value.into_op(target),
        }
    }
}
impl<'scope> ApplyToDom<'scope> for Attr<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, self)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Update(AttrUpdate {
            target,
            data: AttrData::StaticAttr(self),
        })
    }
}
impl<'scope> ApplyToDom<'scope> for bool {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, &Attr::from(*self))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Update(AttrUpdate {
            target,
            data: AttrData::StaticAttr(Attr::from(self)),
        })
    }
}

macro_rules! impl_static_apply { ($($ty:ty),*) => { $(impl<'scope> ApplyToDom<'scope> for $ty { fn apply(&self, element: &DomElement, target: ApplyTarget, context: &MountContext<'scope>) -> SilexResult<()> { let value = self.to_string(); value.apply(element, target, context) } fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> { self.to_string().into_op(target) } })* }; }
impl_static_apply!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl<'scope, T> ApplyToDom<'scope> for Rx<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        if let Some(plan) = T::binding_plan(*self, ReactiveBindingContext::Value(target)) {
            plan.install(element, &context.owner(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        T::binding_plan(self, ReactiveBindingContext::Value(target))
            .map_or(AttrOp::Noop, AttrOp::Reactive)
    }
}

impl<'scope, V: ApplyToDom<'scope> + 'scope> ApplyToDom<'scope> for Option<V> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        if let Some(value) = self {
            value.apply(element, target, context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        self.map_or(AttrOp::Noop, |value| value.into_op(target))
    }
}
impl<'scope, V: ApplyToDom<'scope> + 'scope> ApplyToDom<'scope> for Vec<V> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        for value in self {
            value.apply(element, target.clone(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Sequence(
            self.into_iter()
                .map(|value| value.into_op(target.clone()))
                .collect(),
        )
    }
}
impl<'scope, V: ApplyToDom<'scope> + 'scope, const N: usize> ApplyToDom<'scope> for [V; N] {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        for value in self {
            value.apply(element, target.clone(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Sequence(
            self.into_iter()
                .map(|value| value.into_op(target.clone()))
                .collect(),
        )
    }
}

impl<'scope, K, T> ApplyToDom<'scope> for (K, Rx<'scope, T>)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key = self.0.clone().into();
        if let Some(plan) = T::binding_plan(self.1, ReactiveBindingContext::Pair { key, target }) {
            plan.install(element, &context.owner(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        T::binding_plan(
            self.1,
            ReactiveBindingContext::Pair {
                key: self.0.into(),
                target,
            },
        )
        .map_or(AttrOp::Noop, AttrOp::Reactive)
    }
}
impl<'scope, K> ApplyToDom<'scope> for (K, String)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key = self.0.clone().into();
        let target = if target == ApplyTarget::Apply {
            ApplyTarget::attr(key)
        } else {
            target
        };
        apply_attr_target(context.dom(), element, &target, &Attr::from(self.1.clone()))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let key: Cow<'static, str> = self.0.into();
        let target = if target == ApplyTarget::Apply {
            ApplyTarget::attr(key.clone())
        } else {
            target
        };
        match target {
            ApplyTarget::Style => AttrOp::static_styles(vec![(key, Cow::Owned(self.1))]),
            ApplyTarget::Class => AttrOp::static_class(Cow::Owned(self.1)),
            target => AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::StaticAttr(Attr::from(self.1)),
            }),
        }
    }
}
impl<'scope, K> ApplyToDom<'scope> for (K, &'static str)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key: Cow<'static, str> = self.0.clone().into();
        let target = if target == ApplyTarget::Apply {
            ApplyTarget::attr(key)
        } else {
            target
        };
        apply_attr_target(context.dom(), element, &target, &Attr::from(self.1))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        (self.0, self.1.to_string()).into_op(target)
    }
}
impl<'scope, K> ApplyToDom<'scope> for (K, bool)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key: Cow<'static, str> = self.0.clone().into();
        if target == ApplyTarget::Class {
            let mut classes = HashSet::new();
            if self.1 {
                classes.insert(key.to_string());
            }
            set_classes(context.dom(), element, &classes)
        } else {
            let target = if target == ApplyTarget::Apply {
                ApplyTarget::attr(key)
            } else {
                target
            };
            apply_attr_target(context.dom(), element, &target, &Attr::from(self.1))
        }
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let key: Cow<'static, str> = self.0.into();
        if target == ApplyTarget::Class {
            if self.1 {
                AttrOp::static_class(key)
            } else {
                AttrOp::Noop
            }
        } else {
            AttrOp::Update(AttrUpdate {
                target: if target == ApplyTarget::Apply {
                    ApplyTarget::attr(key)
                } else {
                    target
                },
                data: AttrData::StaticAttr(Attr::from(self.1)),
            })
        }
    }
}

/// 擦除后的属性组。
#[derive(Clone, Default)]
pub struct AttributeGroup<'scope>(pub Vec<AttrOp<'scope>>);

pub fn group<'scope, I>(items: I) -> AttributeGroup<'scope>
where
    I: IntoIterator,
    I::Item: ApplyToDom<'scope> + 'scope,
{
    AttributeGroup(
        items
            .into_iter()
            .map(|item| item.into_op(ApplyTarget::Apply))
            .collect(),
    )
}

pub trait IntoStorable<'scope> {
    type Stored: ApplyToDom<'scope> + 'scope;
    fn into_storable(self) -> Self::Stored;
}
impl<'scope, 'a: 'scope> IntoStorable<'scope> for &'a str {
    type Stored = &'a str;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope> IntoStorable<'scope> for &String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self.clone()
    }
}
impl<'scope> IntoStorable<'scope> for String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, 'a: 'scope> IntoStorable<'scope> for Cow<'a, str> {
    type Stored = Cow<'a, str>;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope> IntoStorable<'scope> for bool {
    type Stored = bool;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
macro_rules! impl_storable { ($($ty:ty),*) => { $(impl<'scope> IntoStorable<'scope> for $ty { type Stored = $ty; fn into_storable(self) -> Self::Stored { self } })* }; }
impl_storable!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);
impl<'scope, T> IntoStorable<'scope> for Rx<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::ReadSignal<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::Signal<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::Computed<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::StoredValue<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope> IntoStorable<'scope> for Attr<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope> IntoStorable<'scope> for AttrOp<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, K, V> IntoStorable<'scope> for (K, V)
where
    K: IntoStorable<'scope>,
    V: IntoStorable<'scope>,
    (K::Stored, V::Stored): ApplyToDom<'scope> + 'scope,
{
    type Stored = (K::Stored, V::Stored);
    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_storable())
    }
}
impl<'scope, V: IntoStorable<'scope>, const N: usize> IntoStorable<'scope> for [V; N]
where
    V::Stored: 'scope,
{
    type Stored = [V::Stored; N];
    fn into_storable(self) -> Self::Stored {
        self.map(IntoStorable::into_storable)
    }
}
impl<'scope, V: IntoStorable<'scope>> IntoStorable<'scope> for Option<V>
where
    V::Stored: 'scope,
{
    type Stored = Option<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.map(IntoStorable::into_storable)
    }
}
impl<'scope, V: IntoStorable<'scope>> IntoStorable<'scope> for Vec<V>
where
    V::Stored: 'scope,
{
    type Stored = Vec<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.into_iter().map(IntoStorable::into_storable).collect()
    }
}
impl<'scope> IntoStorable<'scope> for AttributeGroup<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, 'a, T> IntoStorable<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: Clone + IntoStorable<'scope>,
    T::Stored: 'scope,
{
    type Stored = T::Stored;
    fn into_storable(self) -> Self::Stored {
        match self {
            Prop::Owned(value) => value.into_storable(),
            Prop::Borrowed(value) => value.clone().into_storable(),
        }
    }
}

pub trait AttributeBuilder<'scope>: Sized {
    fn build_attribute<V>(self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>;
    fn build_event<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, M> + Clone + 'scope;
    fn attr<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::attr(name), value)
    }

    fn prop<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::prop(name), value)
    }

    fn on<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(event, callback)
    }

    fn apply<V>(self, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Apply, value)
    }
}

pub trait GlobalAttributes<'scope>: AttributeBuilder<'scope> {
    fn id(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("id", value)
    }

    fn class(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("class", value)
    }

    fn style(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("style", value)
    }

    fn title(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("title", value)
    }

    fn lang(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("lang", value)
    }

    fn dir(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("dir", value)
    }

    fn tabindex(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("tabindex", value)
    }

    fn hidden(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("hidden", value)
    }
}
impl<'scope, T: AttributeBuilder<'scope>> GlobalAttributes<'scope> for T {}

pub trait AriaAttributes<'scope>: AttributeBuilder<'scope> {
    fn role(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("role", value)
    }

    fn aria_label(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-label", value)
    }

    fn aria_hidden(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-hidden", value)
    }

    fn aria_expanded(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-expanded", value)
    }

    fn aria_controls(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-controls", value)
    }

    fn aria_disabled(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-disabled", value)
    }

    fn aria_checked(self, value: impl IntoStorable<'scope>) -> Self {
        self.attr("aria-checked", value)
    }
}
impl<'scope, T: AttributeBuilder<'scope>> AriaAttributes<'scope> for T {}

pub trait GlobalEventAttributes<'scope>: AttributeBuilder<'scope> {
    fn classes<V>(self, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Class, value)
    }

    fn class_toggle<C>(self, name: &str, condition: C) -> Self
    where
        (String, C): IntoStorable<'scope>,
    {
        self.build_attribute(ApplyTarget::Class, (name.to_string(), condition))
    }

    fn node_ref(self, node_ref: NodeRef<'scope>) -> Self {
        self.apply(AttrOp::new_scoped(move |element, context| {
            let owner = context.owner();
            let binding = node_ref
                .bind_for_mount(element.node().clone())
                .map_err(SilexError::from)?;
            owner.on_cleanup(
                Box::new(move || binding.clear_if_current().map(|_| ()).map_err(Into::into)),
                context.error_handler(),
            )
        }))
    }

    fn on_click<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::click, callback)
    }

    fn on_input<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::input, callback)
    }

    /// 将表单控件的当前 value 与可写响应式值建立双向绑定。
    fn bind_value<T, S>(self, signal: S) -> Self
    where
        T: AsRef<str> + From<String> + Clone + PartialEq + 'scope,
        S: IntoStorable<'scope> + RxGet<Owned = T> + RxWrite<Owned = T> + Clone + 'scope,
    {
        let signal_for_input = signal.clone();
        self.on_input(move |event: DomEvent| {
            if let Some(value) = event.input_value() {
                signal_for_input.set(T::from(value))?;
            }
            Ok(())
        })
        .prop("value", signal)
    }

    fn on_change<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::change, callback)
    }

    fn on_pointer_down<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::pointerdown, callback)
    }

    fn on_pointer_move<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::pointermove, callback)
    }

    fn on_pointer_up<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::pointerup, callback)
    }

    fn on_pointer_cancel<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::pointercancel, callback)
    }

    fn on_mouse_enter<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::mouseenter, callback)
    }

    fn on_mouse_leave<F, M>(self, callback: F) -> Self
    where
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.build_event(crate::event::mouseleave, callback)
    }
}
impl<'scope, T: AttributeBuilder<'scope>> GlobalEventAttributes<'scope> for T {}

pub fn consolidate_attributes<'scope>(attrs: Vec<AttrOp<'scope>>) -> Vec<AttrOp<'scope>> {
    #[derive(Default)]
    struct Consolidation<'scope> {
        classes: Vec<Cow<'scope, str>>,
        toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
        class_reactive: Vec<ReactiveBindingPlan<'scope>>,
        styles: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
        style_properties: Vec<ReactiveBindingPlan<'scope>>,
        style_sheets: Vec<ReactiveBindingPlan<'scope>>,
        result: Vec<AttrOp<'scope>>,
    }

    impl<'scope> Consolidation<'scope> {
        fn process(&mut self, op: AttrOp<'scope>) {
            match op {
                AttrOp::Sequence(values) => {
                    for value in values {
                        self.process(value);
                    }
                }
                AttrOp::CombinedClasses(value) => {
                    self.classes.extend(value.statics);
                    self.toggles.extend(value.toggles);
                    self.class_reactive.extend(value.reactives);
                }
                AttrOp::CombinedStyles(value) => {
                    self.styles.extend(value.statics);
                    self.style_properties.extend(value.properties);
                    self.style_sheets.extend(value.sheets);
                }
                AttrOp::Reactive(plan) => match &plan.target {
                    ReactiveBindingTarget::ClassToggle(name) => {
                        self.toggles.push((name.clone(), plan));
                    }
                    ReactiveBindingTarget::DynamicClasses => self.class_reactive.push(plan),
                    ReactiveBindingTarget::StyleProperty(_) => self.style_properties.push(plan),
                    ReactiveBindingTarget::DynamicStyle => self.style_sheets.push(plan),
                    _ => self.result.push(AttrOp::Reactive(plan)),
                },
                AttrOp::Noop => {}
                other => self.result.push(other),
            }
        }

        fn finish(self) -> Vec<AttrOp<'scope>> {
            let Self {
                classes,
                toggles,
                class_reactive,
                styles,
                style_properties,
                style_sheets,
                mut result,
            } = self;
            if !classes.is_empty() || !toggles.is_empty() || !class_reactive.is_empty() {
                result.insert(
                    0,
                    AttrOp::CombinedClasses(CombinedClasses {
                        statics: classes,
                        toggles,
                        reactives: class_reactive,
                    }),
                );
            }
            if !styles.is_empty() || !style_properties.is_empty() || !style_sheets.is_empty() {
                result.insert(
                    usize::from(!result.is_empty()),
                    AttrOp::CombinedStyles(CombinedStyles {
                        statics: styles,
                        properties: style_properties,
                        sheets: style_sheets,
                    }),
                );
            }
            result
        }
    }

    let mut consolidation = Consolidation::default();
    for attr in attrs {
        consolidation.process(attr);
    }
    consolidation.finish()
}

#[macro_export]
macro_rules! group { ($($attr:expr),* $(,)?) => { $crate::AttributeGroup(vec![$($crate::attribute::ApplyToDom::into_op($attr, $crate::ApplyTarget::Apply)),*]) }; }
