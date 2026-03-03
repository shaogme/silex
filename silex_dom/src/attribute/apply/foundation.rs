use std::borrow::Cow;
use wasm_bindgen::JsValue;
use web_sys::Element as WebElem;

use crate::attribute::op::{
    AttrData, AttrOp, AttrTarget, apply_immediate_bool_internal, get_style_decl, parse_style_str,
    set_string_property_internal,
};

// --- Apply Target Enum ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyTarget<'a> {
    /// Standard attributes: `id`, `href`, `src`. Also `class` and `style` when called as attributes.
    Attr(&'a str),
    /// Direct DOM Property (JS object property): `value`, `checked`, `muted` etc.
    Prop(&'a str),
    /// Specialized `.class(...)` call
    Class,
    /// Specialized `.style(...)` call
    Style,
    /// Generic application (e.g. mixins, theme variables)
    Apply,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedApplyTarget {
    Attr(Cow<'static, str>),
    Prop(Cow<'static, str>),
    Class,
    Style,
    Apply,
}

impl<'a> From<ApplyTarget<'a>> for OwnedApplyTarget {
    fn from(target: ApplyTarget<'a>) -> Self {
        match target {
            ApplyTarget::Attr(n) => OwnedApplyTarget::Attr(Cow::Owned(n.to_string())),
            ApplyTarget::Prop(n) => OwnedApplyTarget::Prop(Cow::Owned(n.to_string())),
            ApplyTarget::Class => OwnedApplyTarget::Class,
            ApplyTarget::Style => OwnedApplyTarget::Style,
            ApplyTarget::Apply => OwnedApplyTarget::Apply,
        }
    }
}

impl<'a> From<&'a OwnedApplyTarget> for ApplyTarget<'a> {
    fn from(target: &'a OwnedApplyTarget) -> Self {
        match target {
            OwnedApplyTarget::Attr(n) => ApplyTarget::Attr(n),
            OwnedApplyTarget::Prop(n) => ApplyTarget::Prop(n),
            OwnedApplyTarget::Class => ApplyTarget::Class,
            OwnedApplyTarget::Style => ApplyTarget::Style,
            OwnedApplyTarget::Apply => ApplyTarget::Apply,
        }
    }
}

// --- Traits ---

/// Any type that can be applied as an HTML attribute, class, or style.
/// Replaces AttributeValue, ApplyClass, ApplyStyle.
pub trait ApplyToDom {
    fn apply(&self, el: &WebElem, target: ApplyTarget);

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp
    where
        Self: Sized + 'static,
    {
        AttrOp::Custom(std::rc::Rc::new(move |el| {
            self.apply(el, ApplyTarget::from(&target));
        }))
    }
}

pub trait ReactiveApply {
    fn apply_to_dom(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) where
        Self: Sized;

    fn apply_pair(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) where
        Self: Sized,
    {
        let _ = (rx, key, el, target);
    }

    fn into_op_reactive(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        target: OwnedApplyTarget,
    ) -> Option<AttrOp>
    where
        Self: Sized,
    {
        let _ = (rx, target);
        None
    }
}

// --- Basic Traits & Static Implementations ---

impl ApplyToDom for AttrOp {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        self.clone().apply(el);
    }

    fn into_op(self, _target: OwnedApplyTarget) -> AttrOp {
        self
    }
}

impl ApplyToDom for fn(&WebElem) {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        (self)(el);
    }

    fn into_op(self, _target: OwnedApplyTarget) -> AttrOp {
        AttrOp::Custom(std::rc::Rc::new(self))
    }
}

impl ApplyToDom for std::rc::Rc<dyn Fn(&WebElem)> {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        (self)(el);
    }

    fn into_op(self, _target: OwnedApplyTarget) -> AttrOp {
        AttrOp::Custom(self.clone())
    }
}

pub(crate) fn apply_immediate_string(el: &WebElem, target: ApplyTarget, value: &str) {
    match target {
        ApplyTarget::Attr(n) => set_string_property_internal(el, n, value, false),
        ApplyTarget::Prop(n) => set_string_property_internal(el, n, value, true),
        ApplyTarget::Class => set_string_property_internal(el, "class", value, false),
        ApplyTarget::Style => set_string_property_internal(el, "style", value, false),
        ApplyTarget::Apply => {}
    }
}

pub(crate) fn apply_immediate_bool(el: &WebElem, target: ApplyTarget, value: bool) {
    if let ApplyTarget::Attr(name) = target {
        apply_immediate_bool_internal(el, name, value, false);
    } else if let ApplyTarget::Prop(name) = target {
        apply_immediate_bool_internal(el, name, value, true);
    }
}

pub(crate) fn apply_static_pair(el: &WebElem, target: ApplyTarget, key: &str, value: &str) {
    match target {
        ApplyTarget::Style => {
            if let Some(style) = get_style_decl(el) {
                let _ = style.set_property(key, value);
            }
        }
        ApplyTarget::Attr("style") => {
            if let Some(style) = get_style_decl(el) {
                let _ = style.set_property(key, value);
            }
        }
        _ => {
            apply_immediate_string(el, target, value);
        }
    }
}

pub(crate) fn apply_primitive_static_internal(el: &WebElem, target: ApplyTarget, value: String) {
    apply_immediate_string(el, target, &value);
}

impl ApplyToDom for &'static str {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }
    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        match target {
            OwnedApplyTarget::Attr(name) => AttrOp::Update {
                name,
                target: AttrTarget::Attr,
                data: AttrData::StaticString(self.into()),
            },
            OwnedApplyTarget::Prop(name) => AttrOp::Update {
                name,
                target: AttrTarget::Prop,
                data: AttrData::StaticJs(JsValue::from_str(self)),
            },
            OwnedApplyTarget::Class => AttrOp::SetStaticClasses(vec![self.into()]),
            OwnedApplyTarget::Style => {
                AttrOp::SetStaticStyles(parse_style_str(self).into_iter().collect())
            }
            OwnedApplyTarget::Apply => AttrOp::Custom(std::rc::Rc::new(move |el| {
                apply_immediate_string(el, ApplyTarget::Apply, self);
            })),
        }
    }
}

impl ApplyToDom for String {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        match target {
            OwnedApplyTarget::Attr(name) => AttrOp::Update {
                name,
                target: AttrTarget::Attr,
                data: AttrData::StaticString(self.into()),
            },
            OwnedApplyTarget::Prop(name) => AttrOp::Update {
                name,
                target: AttrTarget::Prop,
                data: AttrData::StaticJs(JsValue::from_str(&self)),
            },
            OwnedApplyTarget::Class => AttrOp::SetStaticClasses(
                self.split_whitespace()
                    .map(|s| Cow::Owned(s.to_string()))
                    .collect(),
            ),
            OwnedApplyTarget::Style => AttrOp::SetStaticStyles(
                parse_style_str(&self)
                    .into_iter()
                    .map(|(k, v)| (k.into_owned().into(), v.into_owned().into()))
                    .collect(),
            ),
            OwnedApplyTarget::Apply => AttrOp::Custom(std::rc::Rc::new(move |el| {
                apply_immediate_string(el, ApplyTarget::Apply, &self);
            })),
        }
    }
}

impl ApplyToDom for &String {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        self.to_string().into_op(target)
    }
}

impl ApplyToDom for bool {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_bool(el, target, *self);
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        match target {
            OwnedApplyTarget::Attr(name) => AttrOp::Update {
                name,
                target: AttrTarget::Attr,
                data: AttrData::StaticBool(self),
            },
            OwnedApplyTarget::Prop(name) => AttrOp::Update {
                name,
                target: AttrTarget::Prop,
                data: AttrData::StaticBool(self),
            },
            _ => AttrOp::Custom(std::rc::Rc::new(move |el| {
                apply_immediate_bool(el, ApplyTarget::Apply, self);
            })),
        }
    }
}

impl<V: ApplyToDom + 'static> ApplyToDom for Option<V> {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        if let Some(v) = self {
            v.apply(el, target);
        }
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        if let Some(v) = self {
            v.into_op(target)
        } else {
            AttrOp::Noop
        }
    }
}

impl<V: ApplyToDom + 'static> ApplyToDom for Vec<V> {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        for v in self {
            v.apply(el, target);
        }
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        let ops = self
            .into_iter()
            .map(|v| v.into_op(target.clone()))
            .collect();
        AttrOp::Sequence(ops)
    }
}

impl<V: ApplyToDom + 'static, const N: usize> ApplyToDom for [V; N] {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        for v in self {
            v.apply(el, target);
        }
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        let ops = self
            .into_iter()
            .map(|v| v.into_op(target.clone()))
            .collect();
        AttrOp::Sequence(ops)
    }
}

macro_rules! impl_apply_to_dom_for_primitive {
    ($($t:ty),*) => {
        $(
            impl ApplyToDom for $t {
                #[inline]
                fn apply(&self, el: &WebElem, target: ApplyTarget) {
                    apply_primitive_static_internal(el, target, self.to_string());
                }

                fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
                    self.to_string().into_op(target)
                }
            }
        )*
    };
}
impl_apply_to_dom_for_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

// --- Tuples ---

// 响应式元组归一化终点：(K, Rx<T>)
impl<K, T> ApplyToDom for (K, silex_core::Rx<T, silex_core::RxValueKind>)
where
    K: Into<Cow<'static, str>> + Clone,
    T: ReactiveApply + Clone + 'static,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        let (key, rx) = self.clone();
        let el = el.clone();
        let owned_target = OwnedApplyTarget::from(target);
        T::apply_pair(rx, key.into(), el, owned_target);
    }
}

// 静态元组 (Key, StaticValue)
impl<K> ApplyToDom for (K, String)
where
    K: AsRef<str>,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_static_pair(el, target, self.0.as_ref(), &self.1);
    }
}

impl<K> ApplyToDom for (K, &str)
where
    K: AsRef<str>,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_static_pair(el, target, self.0.as_ref(), self.1);
    }
}

impl<K> ApplyToDom for (K, bool)
where
    K: AsRef<str> + Clone,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        let (key, value) = self.clone();
        let owned_target = OwnedApplyTarget::from(target);
        match owned_target {
            OwnedApplyTarget::Class => {
                let list = el.class_list();
                if value {
                    let _ = list.add_1(key.as_ref());
                } else {
                    let _ = list.remove_1(key.as_ref());
                }
            }
            OwnedApplyTarget::Attr(ref n) if n == "class" => {
                let list = el.class_list();
                if value {
                    let _ = list.add_1(key.as_ref());
                } else {
                    let _ = list.remove_1(key.as_ref());
                }
            }
            _ => {
                apply_immediate_bool(el, target, value);
            }
        }
    }
}

// --- Attribute Group Support (Erased Collection) ---

/// 擦除后的属性组。
/// 内部持有一组 AttrOp 指令，避免了递归泛型带来的单态化膨胀。
#[derive(Clone, Default)]
pub struct AttributeGroup(pub Vec<AttrOp>);

/// 创建一个擦除后的属性组。
/// 这里的逻辑是：将所有输入项立即转换为 AttrOp。
/// 默认使用 ApplyTarget::Apply 作为转换上下文。
pub fn group<I>(items: I) -> AttributeGroup
where
    I: IntoIterator,
    I::Item: ApplyToDom + 'static,
{
    let ops = items
        .into_iter()
        .map(|item| item.into_op(OwnedApplyTarget::Apply))
        .collect();
    AttributeGroup(ops)
}

impl ApplyToDom for AttributeGroup {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        for op in &self.0 {
            op.clone().apply(el);
        }
    }

    fn into_op(self, _target: OwnedApplyTarget) -> AttrOp {
        if self.0.is_empty() {
            AttrOp::Noop
        } else if self.0.len() == 1 {
            self.0.into_iter().next().unwrap()
        } else {
            AttrOp::Sequence(self.0)
        }
    }
}
