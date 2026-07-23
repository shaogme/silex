use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsValue;
use web_sys::Element as WebElem;

use silex_core::reactivity::Effect;
use silex_core::traits::RxGet;
use silex_core::{Rx, RxValueKind};

use super::foundation::{ApplyTarget, ApplyToDom, OwnedApplyTarget, ReactiveApply};
use crate::attribute::op::{
    Attr, AttrData, AttrOp, AttrTarget, AttrUpdate, apply_attr_with_target_internal,
    get_style_decl, parse_style_str, set_string_property_internal,
};

// --- Internal Helper Functions (Non-generic to reduce monomorphization) ---

pub(crate) fn derive_string_rx_internal<T: std::fmt::Display + Clone + 'static>(
    rx: silex_core::Rx<T, silex_core::RxValueKind>,
) -> silex_core::Rx<String, silex_core::RxValueKind> {
    silex_core::Rx::derive(Box::new(move || {
        use silex_core::traits::RxGet;
        rx.get().to_string()
    }))
}

pub(crate) fn derive_attr_rx<T: Into<Attr> + Clone + 'static>(
    rx: Rx<T, RxValueKind>,
) -> Rx<Attr, RxValueKind> {
    Rx::derive(Box::new(move || rx.get().into()))
}

pub(crate) fn apply_primitive_reactive_internal(
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
    AttrOp::AddReactiveClasses(rx).apply(&el);
}

fn create_erased_style_effect_internal(
    el: WebElem,
    rx: silex_core::Rx<String, silex_core::RxValueKind>,
) {
    AttrOp::BindReactiveStyleSheet(rx).apply(&el);
}

pub(crate) fn apply_string_reactive_internal(
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
                let value = rx.get();
                set_string_property_internal(&el, &name, &value, true);
            });
        }
        OwnedApplyTarget::Known(kp) => {
            Effect::new(move |_| {
                let value = rx.get();
                apply_attr_with_target_internal(
                    &el,
                    kp.name(),
                    AttrTarget::Known(kp),
                    &Attr::from(value),
                );
            });
        }
        OwnedApplyTarget::Apply => {}
    }
}

pub(crate) fn apply_string_pair_reactive_internal(
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

pub(crate) fn apply_bool_reactive_internal(
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
                let val = rx.get();
                apply_attr_with_target_internal(&el, &name, AttrTarget::Prop, &Attr::from(val));
            });
        }
        OwnedApplyTarget::Known(kp) => {
            Effect::new(move |_| {
                use silex_core::traits::RxGet;
                let val = rx.get();
                apply_attr_with_target_internal(&el, kp.name(), AttrTarget::Known(kp), &Attr::from(val));
            });
        }
        _ => {}
    }
}

pub(crate) fn apply_bool_pair_reactive_internal(
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

// 1. 逻辑型 Rx (Effect) - 用于 on_xxx 属性
// 仅支持擦除后的 Rc<dyn Fn> 类型，以收敛单态化
impl ApplyToDom for silex_core::Rx<std::rc::Rc<dyn Fn(&WebElem)>, silex_core::RxEffectKind> {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        use silex_core::traits::RxRead;
        self.with_untracked(|f| (f)(el));
    }

    fn into_op(self, _target: OwnedApplyTarget) -> AttrOp {
        AttrOp::Custom(std::rc::Rc::new(move |el| {
            use silex_core::traits::RxRead;
            self.with_untracked(|f| (f)(el));
        }))
    }
}

// 2. 响应式原语 (经过 IntoStorable 归一化后的终点)
impl<T> ApplyToDom for silex_core::Rx<T, silex_core::RxValueKind>
where
    T: ReactiveApply + Clone + 'static,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_rx_internal(*self, el, target);
    }

    fn into_op(self, target: OwnedApplyTarget) -> AttrOp {
        if let Some(op) = <T as ReactiveApply>::into_op_reactive(self, target.clone()) {
            op
        } else {
            let rx = self;
            let target_fixed = target.clone();
            AttrOp::Custom(std::rc::Rc::new(move |el| {
                apply_rx_internal(rx, el, ApplyTarget::from(&target_fixed));
            }))
        }
    }
}

// --- ReactiveApply Implementations ---

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
                    AttrOp::Update(AttrUpdate {
                        name,
                        target: AttrTarget::Attr,
                        data: AttrData::ReactiveAttr(derive_attr_rx(rx)),
                    })
                }
            }
            OwnedApplyTarget::Known(kp) => AttrOp::Update(AttrUpdate {
                name: Cow::Borrowed(kp.name()),
                target: AttrTarget::Known(kp),
                data: AttrData::ReactiveAttr(derive_attr_rx(rx)),
            }),
            OwnedApplyTarget::Prop(name) => AttrOp::Update(AttrUpdate {
                name,
                target: AttrTarget::Prop,
                data: AttrData::ReactiveJs({
                    silex_core::Rx::derive(Box::new(move || {
                        use silex_core::traits::RxGet;
                        JsValue::from_str(&rx.get())
                    }))
                }),
            }),
            OwnedApplyTarget::Class => AttrOp::AddReactiveClasses(rx),
            OwnedApplyTarget::Style => AttrOp::BindReactiveStyleSheet(rx),
            OwnedApplyTarget::Apply => {
                let rx_inner = rx;
                AttrOp::Custom(std::rc::Rc::new(move |el| {
                    apply_string_reactive_internal(el.clone(), OwnedApplyTarget::Apply, rx_inner);
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

impl ReactiveApply for Attr {
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: WebElem, target: OwnedApplyTarget) {
        let attr_target = match target {
            OwnedApplyTarget::Known(kp) => AttrTarget::Known(kp),
            OwnedApplyTarget::Prop(_) => AttrTarget::Prop,
            _ => AttrTarget::Attr,
        };
        let name = match target {
            OwnedApplyTarget::Known(kp) => Cow::Borrowed(kp.name()),
            OwnedApplyTarget::Attr(ref n) | OwnedApplyTarget::Prop(ref n) => n.clone(),
            _ => Cow::Borrowed(""),
        };
        if !name.is_empty() {
            Effect::new(move |_| {
                apply_attr_with_target_internal(&el, &name, attr_target, &rx.get());
            });
        }
    }

    fn into_op_reactive(rx: Rx<Self, RxValueKind>, target: OwnedApplyTarget) -> Option<AttrOp> {
        let op = match target {
            OwnedApplyTarget::Known(kp) => AttrOp::Update(AttrUpdate {
                name: Cow::Borrowed(kp.name()),
                target: AttrTarget::Known(kp),
                data: AttrData::ReactiveAttr(rx),
            }),
            OwnedApplyTarget::Attr(name) => AttrOp::Update(AttrUpdate {
                name,
                target: AttrTarget::Attr,
                data: AttrData::ReactiveAttr(rx),
            }),
            OwnedApplyTarget::Prop(name) => AttrOp::Update(AttrUpdate {
                name,
                target: AttrTarget::Prop,
                data: AttrData::ReactiveAttr(rx),
            }),
            _ => {
                let rx_inner = rx;
                let target_clone = target.clone();
                AttrOp::Custom(std::rc::Rc::new(move |el| {
                    <Self as ReactiveApply>::apply_to_dom(
                        rx_inner,
                        el.clone(),
                        target_clone.clone(),
                    );
                }))
            }
        };
        Some(op)
    }
}

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
            OwnedApplyTarget::Attr(name) => AttrOp::Update(AttrUpdate {
                name,
                target: AttrTarget::Attr,
                data: AttrData::ReactiveAttr(derive_attr_rx(rx)),
            }),
            OwnedApplyTarget::Prop(name) => AttrOp::Update(AttrUpdate {
                name,
                target: AttrTarget::Prop,
                data: AttrData::ReactiveAttr(derive_attr_rx(rx)),
            }),
            OwnedApplyTarget::Known(kp) => AttrOp::Update(AttrUpdate {
                name: Cow::Borrowed(kp.name()),
                target: AttrTarget::Known(kp),
                data: AttrData::ReactiveAttr(derive_attr_rx(rx)),
            }),
            _ => {
                let rx_inner = rx;
                let target_clone = target.clone();
                AttrOp::Custom(std::rc::Rc::new(move |el| {
                    apply_bool_reactive_internal(el.clone(), target_clone.clone(), rx_inner);
                }))
            }
        };
        Some(op)
    }
}

// --- Option<T> ReactiveApply ---

fn update_option_class_diff(el: &WebElem, prev: Option<&str>, new_val: Option<&str>) {
    if prev == new_val {
        return;
    }
    let list = el.class_list();
    if let Some(p) = prev {
        for c in p.split_whitespace() {
            if new_val.map_or(true, |n| !n.split_whitespace().any(|t| t == c)) {
                let _ = list.remove_1(c);
            }
        }
    }
    if let Some(n) = new_val {
        for c in n.split_whitespace() {
            if prev.map_or(true, |p| !p.split_whitespace().any(|t| t == c)) {
                let _ = list.add_1(c);
            }
        }
    }
}

fn update_option_style_diff(el: &WebElem, prev: Option<&str>, new_val: Option<&str>) {
    if prev == new_val {
        return;
    }
    if let Some(style) = get_style_decl(el) {
        if let Some(p) = prev {
            for (k, _) in parse_style_str(p) {
                if new_val.map_or(true, |n| !parse_style_str(n).iter().any(|(nk, _)| nk == &k)) {
                    let _ = style.remove_property(&k);
                }
            }
        }
        if let Some(n) = new_val {
            for (k, v) in parse_style_str(n) {
                let _ = style.set_property(&k, &v);
            }
        }
    }
}

// --- Option<T> ReactiveApply ---

pub(crate) fn apply_option_reactive_internal<T>(
    el: WebElem,
    target: OwnedApplyTarget,
    rx: silex_core::Rx<Option<T>, silex_core::RxValueKind>,
) where
    T: std::fmt::Display + Clone + 'static,
{
    let prev_val = Rc::new(RefCell::new(None::<String>));

    Effect::new(move |_| {
        use silex_core::traits::RxGet;
        let opt = rx.get();
        let new_val = opt.map(|v| v.to_string());
        let mut prev = prev_val.borrow_mut();

        match target {
            OwnedApplyTarget::Attr(ref name) => {
                if name == "class" {
                    update_option_class_diff(&el, prev.as_deref(), new_val.as_deref());
                } else if name == "style" {
                    update_option_style_diff(&el, prev.as_deref(), new_val.as_deref());
                } else {
                    match new_val {
                        Some(ref v) => {
                            set_string_property_internal(&el, name, v, false);
                        }
                        None => {
                            let _ = el.remove_attribute(name);
                        }
                    }
                }
            }
            OwnedApplyTarget::Prop(ref name) => {
                let attr = match &new_val {
                    Some(v) => Attr::from(v.clone()),
                    None => Attr::Removed,
                };
                apply_attr_with_target_internal(&el, name, AttrTarget::Prop, &attr);
            }
            OwnedApplyTarget::Known(kp) => {
                let attr = match &new_val {
                    Some(v) => Attr::from(v.clone()),
                    None => Attr::Removed,
                };
                apply_attr_with_target_internal(&el, kp.name(), AttrTarget::Known(kp), &attr);
            }
            OwnedApplyTarget::Class => {
                update_option_class_diff(&el, prev.as_deref(), new_val.as_deref());
            }
            OwnedApplyTarget::Style => {
                update_option_style_diff(&el, prev.as_deref(), new_val.as_deref());
            }
            OwnedApplyTarget::Apply => {}
        }

        *prev = new_val;
    });
}

pub(crate) fn apply_option_pair_reactive_internal<T>(
    el: WebElem,
    key: Cow<'static, str>,
    target: OwnedApplyTarget,
    rx: silex_core::Rx<Option<T>, silex_core::RxValueKind>,
) where
    T: std::fmt::Display + Clone + 'static,
{
    let is_class = matches!(target, OwnedApplyTarget::Class)
        || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "class");
    let is_style = matches!(target, OwnedApplyTarget::Style)
        || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "style");

    if is_class {
        let list = el.class_list();
        Effect::new(move |_| {
            use silex_core::traits::RxGet;
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
    } else if is_style && let Some(style) = get_style_decl(&el) {
        Effect::new(move |_| {
            use silex_core::traits::RxGet;
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
    T: std::fmt::Display + Clone + 'static,
{
    fn apply_to_dom(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        apply_option_reactive_internal(el, target, rx);
    }

    fn apply_pair(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        apply_option_pair_reactive_internal(el, key, target, rx);
    }
}
