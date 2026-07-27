use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::rc::Rc;

use silex_core::reactivity::Effect;
use silex_core::traits::{RxGet, RxRead};
use silex_core::{Rx, RxEffectKind, RxValueKind};
use wasm_bindgen::JsValue;
use web_sys::Element as WebElem;

use super::foundation::{ApplyTarget, ApplyToDom, ReactiveApply};
use crate::attribute::op::{
    Attr, AttrData, AttrOp, AttrUpdate, apply_attr_with_target_internal, get_style_decl,
    parse_style_str, set_string_property_internal,
};

// --- Internal Helper Functions (Non-generic to reduce monomorphization) ---

pub(crate) fn derive_string_rx_internal<T: Display + Clone + 'static>(
    rx: Rx<T, RxValueKind>,
) -> Rx<String, RxValueKind> {
    Rx::derive(Box::new(move || rx.get().to_string()))
}

pub(crate) fn apply_primitive_reactive_internal<T: Display + Clone + 'static>(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<T, RxValueKind>,
) {
    match target {
        ApplyTarget::Class => {
            create_erased_class_effect_internal(el, derive_string_rx_internal(rx))
        }
        ApplyTarget::Style => create_raw_style_effect_internal(el, derive_string_rx_internal(rx)),
        ApplyTarget::Attr(ref name) if name == "class" => {
            create_erased_class_effect_internal(el, derive_string_rx_internal(rx));
        }
        ApplyTarget::Attr(ref name) if name == "style" => {
            create_raw_style_effect_internal(el, derive_string_rx_internal(rx));
        }
        ApplyTarget::Attr(name) => {
            Effect::new(move |_| {
                let value = rx.get().to_string();
                set_string_property_internal(&el, &name, &value, false);
            });
        }
        ApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                let value = rx.get().to_string();
                set_string_property_internal(&el, &name, &value, true);
            });
        }
        ApplyTarget::Known(kp) => {
            Effect::new(move |_| {
                let value = rx.get().to_string();
                apply_attr_with_target_internal(
                    &el,
                    kp.name(),
                    ApplyTarget::Known(kp),
                    &Attr::from(value),
                );
            });
        }
        ApplyTarget::Apply => {}
    }
}

fn create_erased_class_effect_internal(el: WebElem, rx: Rx<String, RxValueKind>) {
    AttrOp::reactive_classes(rx).apply(&el);
}

fn create_raw_style_effect_internal(el: WebElem, rx: Rx<String, RxValueKind>) {
    AttrOp::reactive_stylesheet(rx).apply(&el);
}

pub(crate) fn apply_string_reactive_internal(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<String, RxValueKind>,
) {
    match target {
        ApplyTarget::Class => create_erased_class_effect_internal(el, rx),
        ApplyTarget::Style => create_raw_style_effect_internal(el, rx),
        ApplyTarget::Attr(name) => {
            Effect::new(move |_| {
                let value = rx.get();
                set_string_property_internal(&el, &name, &value, false);
            });
        }
        ApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                let value = rx.get();
                set_string_property_internal(&el, &name, &value, true);
            });
        }
        ApplyTarget::Known(kp) => {
            Effect::new(move |_| {
                let value = rx.get();
                apply_attr_with_target_internal(
                    &el,
                    kp.name(),
                    ApplyTarget::Known(kp),
                    &Attr::from(value),
                );
            });
        }
        ApplyTarget::Apply => {}
    }
}

pub(crate) fn apply_string_pair_reactive_internal(
    el: WebElem,
    key: Cow<'static, str>,
    target: ApplyTarget,
    rx: Rx<String, RxValueKind>,
) {
    if matches!(target, ApplyTarget::Style)
        && let Some(style) = get_style_decl(&el)
    {
        Effect::new(move |_| {
            let _ = style.set_property(&key, &rx.get());
        });
    }
}

pub(crate) fn apply_bool_reactive_internal(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<bool, RxValueKind>,
) {
    match target {
        ApplyTarget::Attr(name) => {
            Effect::new(move |_| {
                let val = rx.get();
                if val {
                    let _ = el.set_attribute(&name, "");
                } else {
                    let _ = el.remove_attribute(&name);
                }
            });
        }
        ApplyTarget::Prop(name) => {
            let target = ApplyTarget::prop(name.clone());
            Effect::new(move |_| {
                let val = rx.get();
                apply_attr_with_target_internal(&el, &name, target.clone(), &Attr::from(val));
            });
        }
        ApplyTarget::Known(kp) => {
            Effect::new(move |_| {
                let val = rx.get();
                apply_attr_with_target_internal(
                    &el,
                    kp.name(),
                    ApplyTarget::Known(kp),
                    &Attr::from(val),
                );
            });
        }
        _ => {}
    }
}

pub(crate) fn apply_bool_pair_reactive_internal(
    el: WebElem,
    key: Cow<'static, str>,
    rx: Rx<bool, RxValueKind>,
) {
    let list = el.class_list();
    Effect::new(move |_| {
        if rx.get() {
            let _ = list.add_1(&key);
        } else {
            let _ = list.remove_1(&key);
        }
    });
}

pub(crate) fn apply_rx_internal<T>(rx: Rx<T, RxValueKind>, el: &WebElem, target: ApplyTarget)
where
    T: ReactiveApply + 'static,
{
    T::apply_to_dom(rx, el.clone(), target);
}

// 1. 逻辑型 Rx (Effect) - 用于 on_xxx 属性
// 仅支持擦除后的 Rc<dyn Fn> 类型，以收敛单态化
impl ApplyToDom for Rx<Rc<dyn Fn(&WebElem)>, RxEffectKind> {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        self.with_untracked(|f| (f)(el));
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp {
        AttrOp::Custom(Rc::new(move |el| {
            self.with_untracked(|f| (f)(el));
        }))
    }
}

// 2. 响应式原语 (经过 IntoStorable 归一化后的终点)
impl<T> ApplyToDom for Rx<T, RxValueKind>
where
    T: ReactiveApply + Clone + 'static,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_rx_internal(*self, el, target);
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp {
        if let Some(op) = <T as ReactiveApply>::into_op_reactive(self, target.clone()) {
            op
        } else {
            let rx = self;
            AttrOp::Custom(Rc::new(move |el| {
                apply_rx_internal(rx, el, target.clone());
            }))
        }
    }
}

// --- ReactiveApply Implementations ---

impl ReactiveApply for String {
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: ApplyTarget) {
        apply_string_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
    ) {
        apply_string_pair_reactive_internal(el, key, target, rx);
    }

    fn into_op_reactive(rx: Rx<Self, RxValueKind>, target: ApplyTarget) -> Option<AttrOp> {
        let op = match target {
            ApplyTarget::Attr(name) => AttrOp::Update(AttrUpdate {
                target: ApplyTarget::Attr(name),
                data: AttrData::ReactiveString(rx),
            }),
            ApplyTarget::Known(kp) => AttrOp::Update(AttrUpdate {
                target: ApplyTarget::Known(kp),
                data: AttrData::ReactiveString(rx),
            }),
            ApplyTarget::Prop(name) => AttrOp::Update(AttrUpdate {
                target: ApplyTarget::Prop(name),
                data: AttrData::ReactiveJs({
                    Rx::derive(Box::new(move || JsValue::from_str(&rx.get())))
                }),
            }),
            ApplyTarget::Class => AttrOp::reactive_classes(rx),
            ApplyTarget::Style => AttrOp::reactive_stylesheet(rx),
            ApplyTarget::Apply => {
                let rx_inner = rx;
                AttrOp::Custom(Rc::new(move |el| {
                    apply_string_reactive_internal(el.clone(), ApplyTarget::Apply, rx_inner);
                }))
            }
        };
        Some(op)
    }

    fn into_op_pair_reactive(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp> {
        if matches!(target, ApplyTarget::Style) {
            Some(AttrOp::style_property(key, rx))
        } else {
            None
        }
    }
}

impl ReactiveApply for &'static str {
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: ApplyTarget) {
        apply_primitive_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
    ) {
        let string_rx = derive_string_rx_internal(rx);
        apply_string_pair_reactive_internal(el, key, target, string_rx);
    }

    fn into_op_pair_reactive(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp> {
        let string_rx = derive_string_rx_internal(rx);
        <String as ReactiveApply>::into_op_pair_reactive(string_rx, key, target)
    }
}

macro_rules! impl_reactive_apply_primitive {
    ($($t:ty),*) => {
        $(
            impl ReactiveApply for $t {
                fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: ApplyTarget) {
                    apply_primitive_reactive_internal(el, target, rx);
                }
                fn apply_pair(rx: Rx<Self, RxValueKind>, key: Cow<'static, str>, el: WebElem, target: ApplyTarget) {
                    let string_rx = derive_string_rx_internal(rx);
                    apply_string_pair_reactive_internal(el, key, target, string_rx);
                }
                fn into_op_reactive(rx: Rx<Self, RxValueKind>, target: ApplyTarget) -> Option<AttrOp> {
                    let string_rx = derive_string_rx_internal(rx);
                    <String as ReactiveApply>::into_op_reactive(string_rx, target)
                }
                fn into_op_pair_reactive(rx: Rx<Self, RxValueKind>, key: Cow<'static, str>, target: ApplyTarget) -> Option<AttrOp> {
                    let string_rx = derive_string_rx_internal(rx);
                    <String as ReactiveApply>::into_op_pair_reactive(string_rx, key, target)
                }
            }
        )*
    };
}

impl_reactive_apply_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl ReactiveApply for Attr {
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: ApplyTarget) {
        if let Some(name) = target.name() {
            Effect::new(move |_| {
                apply_attr_with_target_internal(&el, &name, target.clone(), &rx.get());
            });
        }
    }

    fn into_op_reactive(rx: Rx<Self, RxValueKind>, target: ApplyTarget) -> Option<AttrOp> {
        let op = match target {
            ApplyTarget::Known(_) | ApplyTarget::Attr(_) | ApplyTarget::Prop(_) => {
                AttrOp::Update(AttrUpdate {
                    target,
                    data: AttrData::ReactiveAttr(rx),
                })
            }
            _ => {
                let rx_inner = rx;
                AttrOp::Custom(Rc::new(move |el| {
                    <Self as ReactiveApply>::apply_to_dom(rx_inner, el.clone(), target.clone());
                }))
            }
        };
        Some(op)
    }
}

impl ReactiveApply for bool {
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: ApplyTarget) {
        apply_bool_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
    ) {
        if matches!(target, ApplyTarget::Class) {
            apply_bool_pair_reactive_internal(el, key, rx);
        }
    }

    fn into_op_reactive(rx: Rx<Self, RxValueKind>, target: ApplyTarget) -> Option<AttrOp> {
        let op = match target {
            ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                AttrOp::Update(AttrUpdate {
                    target,
                    data: AttrData::ReactiveBool(rx),
                })
            }
            _ => {
                let rx_inner = rx;
                AttrOp::Custom(Rc::new(move |el| {
                    apply_bool_reactive_internal(el.clone(), target.clone(), rx_inner);
                }))
            }
        };
        Some(op)
    }

    fn into_op_pair_reactive(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp> {
        if matches!(target, ApplyTarget::Class) {
            Some(AttrOp::class_toggle(key, rx))
        } else {
            None
        }
    }
}

// --- Option<T> ReactiveApply Diff Helpers ---

fn update_option_class_diff(el: &WebElem, prev: Option<&str>, new_val: Option<&str>) {
    if prev == new_val {
        return;
    }
    let list = el.class_list();
    let prev_tokens: HashSet<&str> =
        prev.map_or_else(HashSet::new, |p| p.split_whitespace().collect());
    let new_tokens: HashSet<&str> =
        new_val.map_or_else(HashSet::new, |n| n.split_whitespace().collect());

    for &c in &prev_tokens {
        if !new_tokens.contains(c) {
            let _ = list.remove_1(c);
        }
    }
    for &c in &new_tokens {
        if !prev_tokens.contains(c) {
            let _ = list.add_1(c);
        }
    }
}

fn update_option_style_diff(el: &WebElem, prev: Option<&str>, new_val: Option<&str>) {
    if prev == new_val {
        return;
    }
    if let Some(style) = get_style_decl(el) {
        let prev_map: HashMap<Cow<'_, str>, Cow<'_, str>> = prev
            .map(|p| parse_style_str(p).into_iter().collect())
            .unwrap_or_default();
        let new_map: HashMap<Cow<'_, str>, Cow<'_, str>> = new_val
            .map(|n| parse_style_str(n).into_iter().collect())
            .unwrap_or_default();

        for k in prev_map.keys() {
            if !new_map.contains_key(k) {
                let _ = style.remove_property(k);
            }
        }
        for (k, v) in &new_map {
            if prev_map.get(k) != Some(v) {
                let _ = style.set_property(k, v);
            }
        }
    }
}

// --- Option<T> ReactiveApply ---

pub(crate) fn apply_option_reactive_internal<T>(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<Option<T>, RxValueKind>,
) where
    T: Display + Clone + 'static,
{
    let prev_val = Rc::new(RefCell::new(None::<String>));

    Effect::new(move |_| {
        let opt = rx.get();
        let new_val = opt.map(|v| v.to_string());
        let mut prev = prev_val.borrow_mut();

        match target {
            ApplyTarget::Attr(ref name) => match new_val {
                Some(ref v) => {
                    set_string_property_internal(&el, name, v, false);
                }
                None => {
                    let _ = el.remove_attribute(name);
                }
            },
            ApplyTarget::Prop(ref name) => {
                let attr = match &new_val {
                    Some(v) => Attr::from(v.clone()),
                    None => Attr::Removed,
                };
                apply_attr_with_target_internal(&el, name, target.clone(), &attr);
            }
            ApplyTarget::Known(kp) => {
                let attr = match &new_val {
                    Some(v) => Attr::from(v.clone()),
                    None => Attr::Removed,
                };
                apply_attr_with_target_internal(&el, kp.name(), ApplyTarget::Known(kp), &attr);
            }
            ApplyTarget::Class => {
                update_option_class_diff(&el, prev.as_deref(), new_val.as_deref());
            }
            ApplyTarget::Style => {
                update_option_style_diff(&el, prev.as_deref(), new_val.as_deref());
            }
            ApplyTarget::Apply => {}
        }

        *prev = new_val;
    });
}

pub(crate) fn apply_option_pair_reactive_internal<T>(
    el: WebElem,
    key: Cow<'static, str>,
    target: ApplyTarget,
    rx: Rx<Option<T>, RxValueKind>,
) where
    T: Display + Clone + 'static,
{
    if matches!(target, ApplyTarget::Class) {
        let list = el.class_list();
        Effect::new(move |_| {
            if let Some(val) = rx.get() {
                let s = val.to_string();
                if s == "true" || !s.is_empty() {
                    let _ = list.add_1(&key);
                } else {
                    let _ = list.remove_1(&key);
                }
            } else {
                let _ = list.remove_1(&key);
            }
        });
    } else if matches!(target, ApplyTarget::Style)
        && let Some(style) = get_style_decl(&el)
    {
        Effect::new(move |_| {
            if let Some(val) = rx.get() {
                let _ = style.set_property(&key, &val.to_string());
            } else {
                let _ = style.remove_property(&key);
            }
        });
    }
}

impl<T> ReactiveApply for Option<T>
where
    T: Display + Clone + 'static,
{
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: ApplyTarget) {
        apply_option_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
    ) {
        apply_option_pair_reactive_internal(el, key, target, rx);
    }

    fn into_op_reactive(rx: Rx<Self, RxValueKind>, target: ApplyTarget) -> Option<AttrOp> {
        if matches!(target, ApplyTarget::Class) {
            Some(AttrOp::reactive_classes(Rx::derive(Box::new(move || {
                rx.get().map(|v| v.to_string()).unwrap_or_default()
            }))))
        } else if matches!(target, ApplyTarget::Style) {
            Some(AttrOp::reactive_stylesheet(Rx::derive(Box::new(
                move || rx.get().map(|v| v.to_string()).unwrap_or_default(),
            ))))
        } else if matches!(
            target,
            ApplyTarget::Attr(_) | ApplyTarget::Known(_) | ApplyTarget::Prop(_)
        ) {
            let opt_rx = Rx::derive(Box::new(move || rx.get().map(|v| v.to_string())));
            Some(AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::ReactiveOptionString(opt_rx),
            }))
        } else {
            let rx_inner = rx;
            Some(AttrOp::Custom(Rc::new(move |el| {
                apply_option_reactive_internal(el.clone(), target.clone(), rx_inner);
            })))
        }
    }

    fn into_op_pair_reactive(
        rx: Rx<Self, RxValueKind>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp> {
        if matches!(target, ApplyTarget::Class) {
            Some(AttrOp::class_toggle(
                key,
                Rx::derive(Box::new(move || {
                    rx.get()
                        .map(|v| {
                            let s = v.to_string();
                            s == "true" || !s.is_empty()
                        })
                        .unwrap_or(false)
                })),
            ))
        } else if matches!(target, ApplyTarget::Style) {
            Some(AttrOp::style_property(
                key,
                Rx::derive(Box::new(move || {
                    rx.get().map(|v| v.to_string()).unwrap_or_default()
                })),
            ))
        } else {
            None
        }
    }
}
