use silex_core::reactivity::Effect;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use web_sys::Element as WebElem;

use super::op::{
    AttrOp, apply_immediate_bool_internal, get_style_decl, parse_style_str,
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

// --- ApplyToDom Trait ---

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

impl<F> ApplyToDom for F
where
    F: Fn(&WebElem) + 'static,
{
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        (self)(el);
    }
}

// --- 统一响应式应用逻辑 ---

// 1. 逻辑型 Rx (Effect) - 用于 on_xxx 属性
impl<F> ApplyToDom for silex_core::Rx<F, silex_core::RxEffectKind>
where
    F: Fn(&WebElem) + 'static,
{
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        use silex_core::traits::RxRead;
        self.with_untracked(|f| (f)(el));
    }
}

// 2. 响应式原语 (经过 IntoStorable 归一化后的终点)
impl<T> ApplyToDom for silex_core::Rx<T, silex_core::RxValueKind>
where
    T: ReactiveApply + Clone + 'static,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        crate::attribute::apply_rx_internal(self.clone(), el, target);
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        if let Some(op) = <T as ReactiveApply>::into_op_reactive(self.clone(), target.clone()) {
            op
        } else {
            let rx = self.clone();
            AttrOp::Custom(std::rc::Rc::new(move |el| {
                crate::attribute::apply_rx_internal(rx.clone(), el, ApplyTarget::from(&target));
            }))
        }
    }
}

// --- Internal Helper Functions (Non-generic to reduce monomorphization) ---

fn derive_string_rx_internal<T: std::fmt::Display + Clone + 'static>(
    rx: silex_core::Rx<T, silex_core::RxValueKind>,
) -> silex_core::Rx<String, silex_core::RxValueKind> {
    silex_core::Rx::derive(Box::new(move || {
        use silex_core::traits::RxGet;
        rx.get().to_string()
    }))
}

fn apply_primitive_static_internal(el: &WebElem, target: ApplyTarget, value: String) {
    apply_immediate_string(el, target, &value);
}

fn apply_primitive_reactive_internal(
    el: WebElem,
    target: OwnedApplyTarget,
    rx_erased: silex_core::Rx<String, silex_core::RxValueKind>,
) {
    apply_string_reactive_internal(el, target, rx_erased);
}

fn create_erased_class_effect_internal(
    el: WebElem,
    rx: silex_core::Rx<String, silex_core::RxValueKind>,
) {
    let prev_classes = Rc::new(RefCell::new(HashSet::new()));

    Effect::new(move |_| {
        use silex_core::traits::RxGet;
        let value = rx.get();
        let new_classes: HashSet<String> =
            value.split_whitespace().map(|s| s.to_string()).collect();

        let mut prev = prev_classes.borrow_mut();
        let list = el.class_list();

        for c in prev.difference(&new_classes) {
            let _ = list.remove_1(c);
        }

        for c in new_classes.difference(&prev) {
            let _ = list.add_1(c);
        }

        *prev = new_classes;
    });
}

fn create_erased_style_effect_internal(
    el: WebElem,
    rx: silex_core::Rx<String, silex_core::RxValueKind>,
) {
    let prev_keys = Rc::new(RefCell::new(HashSet::<String>::new()));

    Effect::new(move |_| {
        use silex_core::traits::RxGet;
        let value = rx.get();
        let new_style_str = value.as_str();

        if let Some(style) = get_style_decl(&el) {
            let mut prev = prev_keys.borrow_mut();
            let params = parse_style_str(new_style_str);
            let new_keys: HashSet<String> = params.iter().map(|(k, _)| k.to_string()).collect();

            for k in prev.difference(&new_keys) {
                let _ = style.remove_property(k);
            }

            for (k, v) in params {
                let _ = style.set_property(&k, &v);
            }

            *prev = new_keys;
        }
    });
}

fn apply_string_reactive_internal(
    el: WebElem,
    target: OwnedApplyTarget,
    rx: silex_core::Rx<String, silex_core::RxValueKind>,
) {
    match target {
        OwnedApplyTarget::Class => create_erased_class_effect_internal(el, rx),
        OwnedApplyTarget::Style => create_erased_style_effect_internal(el, rx),
        OwnedApplyTarget::Attr(name) => {
            if name == "class" {
                create_erased_class_effect_internal(el, rx);
            } else if name == "style" {
                create_erased_style_effect_internal(el, rx);
            } else {
                Effect::new(move |_| {
                    use silex_core::traits::RxGet;
                    let value = rx.get();
                    set_string_property_internal(&el, &name, &value, false);
                });
            }
        }
        OwnedApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                use silex_core::traits::RxGet;
                let value = rx.get();
                set_string_property_internal(&el, &name, &value, true);
            });
        }
        OwnedApplyTarget::Apply => {}
    }
}

fn apply_string_pair_reactive_internal(
    el: WebElem,
    key: Cow<'static, str>,
    target: OwnedApplyTarget,
    rx: silex_core::Rx<String, silex_core::RxValueKind>,
) {
    let is_style = matches!(target, OwnedApplyTarget::Style)
        || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "style");

    if is_style && let Some(style) = get_style_decl(&el) {
        Effect::new(move |_| {
            use silex_core::traits::RxGet;
            let _ = style.set_property(&key, &rx.get());
        });
    }
}

fn apply_bool_reactive_internal(
    el: WebElem,
    target: OwnedApplyTarget,
    rx: silex_core::Rx<bool, silex_core::RxValueKind>,
) {
    match target {
        OwnedApplyTarget::Attr(name) => {
            Effect::new(move |_| {
                use silex_core::traits::RxGet;
                let val = rx.get();
                if val {
                    let _ = el.set_attribute(&name, "");
                } else {
                    let _ = el.remove_attribute(&name);
                }
            });
        }
        OwnedApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                use silex_core::traits::RxGet;
                let val = rx.get();
                let _ =
                    js_sys::Reflect::set(&el, &JsValue::from_str(&name), &JsValue::from_bool(val));
            });
        }
        _ => {}
    }
}

fn apply_bool_pair_reactive_internal(
    el: WebElem,
    key: Cow<'static, str>,
    rx: silex_core::Rx<bool, silex_core::RxValueKind>,
) {
    let list = el.class_list();
    Effect::new(move |_| {
        use silex_core::traits::RxGet;
        if rx.get() {
            let _ = list.add_1(&key);
        } else {
            let _ = list.remove_1(&key);
        }
    });
}

pub(crate) fn apply_rx_internal<T>(
    rx: silex_core::Rx<T, silex_core::RxValueKind>,
    el: &WebElem,
    target: ApplyTarget,
) where
    T: ReactiveApply + 'static,
{
    let owned_target = OwnedApplyTarget::from(target);
    T::apply_to_dom(rx, el.clone(), owned_target);
}

// --- OwnedApplyTarget & ReactiveApply ---

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

// --- Implementations ---

fn apply_immediate_string(el: &WebElem, target: ApplyTarget, value: &str) {
    match target {
        ApplyTarget::Attr(n) => set_string_property_internal(el, n, value, false),
        ApplyTarget::Prop(n) => set_string_property_internal(el, n, value, true),
        ApplyTarget::Class => set_string_property_internal(el, "class", value, false),
        ApplyTarget::Style => set_string_property_internal(el, "style", value, false),
        ApplyTarget::Apply => {}
    }
}

fn apply_immediate_bool(el: &WebElem, target: ApplyTarget, value: bool) {
    if let ApplyTarget::Attr(name) = target {
        apply_immediate_bool_internal(el, name, value, false);
    } else if let ApplyTarget::Prop(name) = target {
        apply_immediate_bool_internal(el, name, value, true);
    }
}

impl ApplyToDom for &'static str {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, *self);
    }
    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        match target {
            OwnedApplyTarget::Attr(name) => AttrOp::SetStaticAttr {
                name: name.into(),
                value: self.into(),
            },
            OwnedApplyTarget::Prop(name) => AttrOp::SetStaticProp {
                name: name.into(),
                value: JsValue::from_str(self),
            },
            OwnedApplyTarget::Class => AttrOp::SetStaticClasses(vec![self.into()]),
            OwnedApplyTarget::Style => AttrOp::SetStaticStyles(
                parse_style_str(self)
                    .into_iter()
                    .map(|(k, v)| {
                        // 在 into_op 中，self 是 &'static str，所以返回的 Cow 确实是 'static。
                        // 这里我们显式地重建 Cow 以通过编译器检查。
                        let k = match k {
                            Cow::Borrowed(s) => Cow::Borrowed(s),
                            Cow::Owned(s) => Cow::Owned(s),
                        };
                        let v = match v {
                            Cow::Borrowed(s) => Cow::Borrowed(s),
                            Cow::Owned(s) => Cow::Owned(s),
                        };
                        (k, v)
                    })
                    .collect(),
            ),
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
            OwnedApplyTarget::Attr(name) => AttrOp::SetStaticAttr {
                name: name.into(),
                value: self.into(),
            },
            OwnedApplyTarget::Prop(name) => AttrOp::SetStaticProp {
                name: name.into(),
                value: JsValue::from_str(&self),
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
        // String is 'static, so we convert to owned String to satisfy 'static bound
        self.to_string().into_op(target)
    }
}

impl ApplyToDom for bool {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_bool(el, target, *self);
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        match target {
            OwnedApplyTarget::Attr(name) => AttrOp::SetStaticBoolAttr {
                name: name.into(),
                value: self,
            },
            OwnedApplyTarget::Prop(name) => AttrOp::SetStaticBoolProp {
                name: name.into(),
                value: self,
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

impl ReactiveApply for String {
    fn apply_to_dom(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        apply_string_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        apply_string_pair_reactive_internal(el, key, target, rx);
    }

    fn into_op_reactive(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        target: OwnedApplyTarget,
    ) -> Option<AttrOp> {
        let op = match target {
            OwnedApplyTarget::Attr(name) => {
                if name == "class" {
                    AttrOp::AddReactiveClasses(rx)
                } else if name == "style" {
                    AttrOp::BindReactiveStyleSheet(rx)
                } else {
                    AttrOp::BindReactiveAttr {
                        name: name.into(),
                        rx,
                    }
                }
            }
            OwnedApplyTarget::Prop(name) => AttrOp::BindReactiveProp {
                name: name.into(),
                rx: {
                    let rx = rx.clone();
                    silex_core::Rx::derive(Box::new(move || {
                        use silex_core::traits::RxGet;
                        JsValue::from_str(&rx.get())
                    }))
                },
            },
            OwnedApplyTarget::Class => AttrOp::AddReactiveClasses(rx),
            OwnedApplyTarget::Style => AttrOp::BindReactiveStyleSheet(rx),
            OwnedApplyTarget::Apply => {
                let rx_inner = rx.clone();
                AttrOp::Custom(std::rc::Rc::new(move |el| {
                    apply_string_reactive_internal(
                        el.clone(),
                        OwnedApplyTarget::Apply,
                        rx_inner.clone(),
                    );
                }))
            }
        };
        Some(op)
    }
}

impl ReactiveApply for &'static str {
    fn apply_to_dom(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        // 自动转换为 Rx<String> 实现类型擦除
        let string_rx = derive_string_rx_internal(rx);
        apply_primitive_reactive_internal(el, target, string_rx);
    }

    fn apply_pair(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        let string_rx = derive_string_rx_internal(rx);
        apply_string_pair_reactive_internal(el, key, target, string_rx);
    }
}

macro_rules! impl_reactive_apply_primitive {
    ($($t:ty),*) => {
        $(
            impl ReactiveApply for $t {
                fn apply_to_dom(rx: silex_core::Rx<Self, silex_core::RxValueKind>, el: WebElem, target: OwnedApplyTarget) {
                    let string_rx = derive_string_rx_internal(rx);
                    apply_primitive_reactive_internal(el, target, string_rx);
                }
                fn apply_pair(rx: silex_core::Rx<Self, silex_core::RxValueKind>, key: Cow<'static, str>, el: WebElem, target: OwnedApplyTarget) {
                    let string_rx = derive_string_rx_internal(rx);
                    apply_string_pair_reactive_internal(el, key, target, string_rx);
                }
                fn into_op_reactive(rx: silex_core::Rx<Self, silex_core::RxValueKind>, target: OwnedApplyTarget) -> Option<AttrOp> {
                    let string_rx = derive_string_rx_internal(rx);
                    <String as ReactiveApply>::into_op_reactive(string_rx, target)
                }
            }
        )*
    };
}

impl_reactive_apply_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl ReactiveApply for bool {
    fn apply_to_dom(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        apply_bool_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        let is_class = matches!(target, OwnedApplyTarget::Class)
            || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "class");

        if is_class {
            apply_bool_pair_reactive_internal(el, key, rx);
        }
    }

    fn into_op_reactive(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        target: OwnedApplyTarget,
    ) -> Option<AttrOp> {
        let op = match target {
            OwnedApplyTarget::Attr(name) => AttrOp::BindReactiveBoolAttr {
                name: name.into(),
                rx,
            },
            OwnedApplyTarget::Prop(name) => AttrOp::BindReactiveBoolProp {
                name: name.into(),
                rx,
            },
            _ => {
                let rx_inner = rx.clone();
                let target_clone = target.clone();
                AttrOp::Custom(std::rc::Rc::new(move |el| {
                    apply_bool_reactive_internal(
                        el.clone(),
                        target_clone.clone(),
                        rx_inner.clone(),
                    );
                }))
            }
        };
        Some(op)
    }
}

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

fn apply_static_pair(el: &WebElem, target: ApplyTarget, key: &str, value: &str) {
    match target {
        ApplyTarget::Style => {
            if let Some(style) = get_style_decl(el) {
                let _ = style.set_property(key, value);
            }
        }
        ApplyTarget::Attr(ref n) if *n == "style" => {
            if let Some(style) = get_style_decl(el) {
                let _ = style.set_property(key, value);
            }
        }
        _ => {
            apply_immediate_string(el, target, value);
        }
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

// 8. AttributeGroup (Macros)
#[derive(Clone)]
pub struct AttributeGroup<T>(pub T);

pub fn group<T>(t: T) -> AttributeGroup<T> {
    AttributeGroup(t)
}

macro_rules! impl_apply_to_dom_for_group {
    ($($name:ident)+) => {
        impl<$($name: ApplyToDom + 'static),+> ApplyToDom for AttributeGroup<($($name,)+)> {
            fn apply(&self, el: &WebElem, target: ApplyTarget) {
                #[allow(non_snake_case)]
                let ($($name,)+) = &self.0;
                $($name.apply(el, target);)+
            }

            fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
                #[allow(non_snake_case)]
                let ($($name,)+) = self.0;
                AttrOp::Sequence(vec![
                    $($name.into_op(target.clone()),)+
                ])
            }
        }
    };
}

impl_apply_to_dom_for_group!(T1);
impl_apply_to_dom_for_group!(T1 T2);
impl_apply_to_dom_for_group!(T1 T2 T3);
impl_apply_to_dom_for_group!(T1 T2 T3 T4);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6 T7);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6 T7 T8);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
impl_apply_to_dom_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);

// --- Attribute Forwarding Support ---

#[derive(Clone)]
pub struct PendingAttribute {
    pub op: AttrOp,
}

impl<'a> From<&'a OwnedApplyTarget> for ApplyTarget<'a> {
    fn from(target: &'a OwnedApplyTarget) -> Self {
        match target {
            OwnedApplyTarget::Attr(n) => ApplyTarget::Attr(&**n),
            OwnedApplyTarget::Prop(n) => ApplyTarget::Prop(&**n),
            OwnedApplyTarget::Class => ApplyTarget::Class,
            OwnedApplyTarget::Style => ApplyTarget::Style,
            OwnedApplyTarget::Apply => ApplyTarget::Apply,
        }
    }
}

pub fn consolidate_attributes(attrs: Vec<PendingAttribute>) -> Vec<PendingAttribute> {
    let mut consolidated = Vec::new();

    // Class 收集器
    let mut static_classes: Vec<Cow<'static, str>> = Vec::new();
    let mut class_toggles: Vec<(Cow<'static, str>, silex_core::Rx<bool>)> = Vec::new();
    let mut reactive_classes: Vec<silex_core::Rx<String>> = Vec::new();

    // Style 收集器
    let mut static_styles: Vec<(Cow<'static, str>, Cow<'static, str>)> = Vec::new();
    let mut style_props: Vec<(Cow<'static, str>, silex_core::Rx<String>)> = Vec::new();
    let mut style_sheets: Vec<silex_core::Rx<String>> = Vec::new();

    for attr in attrs {
        match attr.op {
            // --- Class 指令收集 ---
            AttrOp::SetStaticClasses(v) => {
                static_classes.extend(v);
            }
            AttrOp::AddClassToggle { name, rx } => {
                class_toggles.push((name, rx));
            }
            AttrOp::AddReactiveClasses(rx) => {
                reactive_classes.push(rx);
            }

            // --- Style 指令收集 ---
            AttrOp::SetStaticStyles(v) => {
                static_styles.extend(v);
            }
            AttrOp::BindStyleProperty { name, rx } => {
                style_props.push((name, rx));
            }
            AttrOp::BindReactiveStyleSheet(rx) => {
                style_sheets.push(rx);
            }

            // --- 通用属性指令 (检查是否为 class/style) ---
            AttrOp::SetStaticAttr { name, value } => {
                if name == "class" {
                    match value {
                        Cow::Borrowed(s) => {
                            for token in s.split_whitespace() {
                                static_classes.push(Cow::Borrowed(token));
                            }
                        }
                        Cow::Owned(s) => {
                            for token in s.split_whitespace() {
                                static_classes.push(token.to_string().into());
                            }
                        }
                    }
                } else if name == "style" {
                    static_styles.extend(
                        parse_style_str(&value)
                            .into_iter()
                            .map(|(k, v)| (k.into_owned().into(), v.into_owned().into())),
                    );
                } else {
                    consolidated.push(PendingAttribute {
                        op: AttrOp::SetStaticAttr { name, value },
                    });
                }
            }

            // --- 其它指令，原样保留 ---
            op => {
                consolidated.push(PendingAttribute { op });
            }
        }
    }

    // 按需生成合并后的 Style 指令
    if !static_styles.is_empty() || !style_props.is_empty() || !style_sheets.is_empty() {
        consolidated.insert(
            0,
            PendingAttribute {
                op: AttrOp::CombinedStyles {
                    statics: static_styles,
                    properties: style_props,
                    sheets: style_sheets,
                },
            },
        );
    }

    // 按需生成合并后的 Class 指令
    if !static_classes.is_empty() || !class_toggles.is_empty() || !reactive_classes.is_empty() {
        consolidated.insert(
            0,
            PendingAttribute {
                op: AttrOp::CombinedClasses {
                    statics: static_classes,
                    toggles: class_toggles,
                    reactives: reactive_classes,
                },
            },
        );
    }

    consolidated
}

impl PendingAttribute {
    pub fn build<V>(value: V, target: OwnedApplyTarget) -> Self
    where
        V: ApplyToDom + 'static,
    {
        let op = value.into_op(target);
        Self { op }
    }

    pub fn apply(&self, el: &WebElem) {
        self.op.clone().apply(el);
    }

    pub fn new_listener(f: impl Fn(&WebElem) + 'static) -> Self {
        Self {
            op: AttrOp::Custom(Rc::new(f)),
        }
    }
}
