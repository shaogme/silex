use silex_core::SilexError;
use silex_core::reactivity::Effect;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Element as WebElem, HtmlElement, SvgElement};

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

    fn into_payload(self) -> AttributePayload
    where
        Self: Sized + 'static,
    {
        AttributePayload::Dynamic(std::rc::Rc::new(move |el, target| {
            self.apply(el, ApplyTarget::from(target));
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

    fn into_payload(self) -> AttributePayload {
        if let Some(payload) = <T as ReactiveApply>::into_payload_reactive(self.clone()) {
            payload
        } else {
            AttributePayload::Dynamic(std::rc::Rc::new(move |el, target| {
                crate::attribute::apply_rx_internal(self.clone(), el, ApplyTarget::from(target));
            }))
        }
    }
}

// --- Internal Helper Functions (Non-generic to reduce monomorphization) ---

fn handle_err(res: Result<(), SilexError>) {
    if let Err(e) = res {
        silex_core::error::handle_error(e);
    }
}

fn get_style_decl(el: &WebElem) -> Option<CssStyleDeclaration> {
    if let Some(e) = el.dyn_ref::<HtmlElement>() {
        Some(e.style())
    } else {
        el.dyn_ref::<SvgElement>().map(|e| e.style())
    }
}

fn parse_style_str(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .filter_map(|rule| {
            let rule = rule.trim();
            if rule.is_empty() {
                None
            } else {
                rule.split_once(':')
                    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            }
        })
        .collect()
}

fn set_string_property_internal(el: &WebElem, name: &str, value: &str, is_prop: bool) {
    if is_prop {
        let _ = js_sys::Reflect::set(el, &JsValue::from_str(name), &JsValue::from_str(value));
    } else {
        match name {
            "class" => el.set_class_name(value),
            "style" => {
                if let Some(style) = get_style_decl(el) {
                    style.set_css_text(value);
                }
            }
            _ => {
                handle_err(
                    el.set_attribute(name, value)
                        .map_err(silex_core::SilexError::from),
                );
            }
        }
    }
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
            let new_keys: HashSet<String> = params.iter().map(|(k, _)| k.clone()).collect();

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
    key: String,
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
    key: String,
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

#[derive(Clone)]
pub enum OwnedApplyTarget {
    Attr(String),
    Prop(String),
    Class,
    Style,
    Apply,
}

impl<'a> From<ApplyTarget<'a>> for OwnedApplyTarget {
    fn from(target: ApplyTarget<'a>) -> Self {
        match target {
            ApplyTarget::Attr(n) => OwnedApplyTarget::Attr(n.to_string()),
            ApplyTarget::Prop(n) => OwnedApplyTarget::Prop(n.to_string()),
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
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) where
        Self: Sized,
    {
        let _ = (rx, key, el, target);
    }

    fn into_payload_reactive(
        _rx: silex_core::Rx<Self, silex_core::RxValueKind>,
    ) -> Option<AttributePayload>
    where
        Self: Sized,
    {
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
    match target {
        ApplyTarget::Attr(name) => {
            if value {
                let _ = el.set_attribute(name, "");
            } else {
                let _ = el.remove_attribute(name);
            }
        }
        ApplyTarget::Prop(name) => {
            let _ = js_sys::Reflect::set(el, &JsValue::from_str(name), &JsValue::from_bool(value));
        }
        _ => {}
    }
}

impl ApplyToDom for &str {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, *self);
    }

    fn into_payload(self) -> AttributePayload {
        AttributePayload::StaticString(self.to_string())
    }
}

impl ApplyToDom for String {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }

    fn into_payload(self) -> AttributePayload {
        AttributePayload::StaticString(self)
    }
}

impl ApplyToDom for &String {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }

    fn into_payload(self) -> AttributePayload {
        AttributePayload::StaticString(self.to_string())
    }
}

impl ApplyToDom for bool {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_bool(el, target, *self);
    }

    fn into_payload(self) -> AttributePayload {
        AttributePayload::StaticBool(self)
    }
}

impl<T: ApplyToDom> ApplyToDom for Option<T> {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        if let Some(val) = self {
            val.apply(el, target);
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
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        apply_string_pair_reactive_internal(el, key, target, rx);
    }

    fn into_payload_reactive(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
    ) -> Option<AttributePayload> {
        Some(AttributePayload::ReactiveString(rx))
    }
}

impl ReactiveApply for &'static str {
    fn apply_to_dom(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        use silex_core::traits::RxGet;
        // 自动转换为 Rx<String> 实现类型擦除
        let string_rx = silex_core::Rx::derive(Box::new(move || rx.get().to_string()));
        apply_string_reactive_internal(el, target, string_rx);
    }

    fn apply_pair(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        use silex_core::traits::RxGet;
        let string_rx = silex_core::Rx::derive(Box::new(move || rx.get().to_string()));
        apply_string_pair_reactive_internal(el, key, target, string_rx);
    }
}

macro_rules! impl_reactive_apply_primitive {
    ($($t:ty),*) => {
        $(
            impl ReactiveApply for $t {
                fn apply_to_dom(rx: silex_core::Rx<Self, silex_core::RxValueKind>, el: WebElem, target: OwnedApplyTarget) {
                    use silex_core::traits::RxGet;
                    let string_rx = silex_core::Rx::derive(Box::new(move || rx.get().to_string()));
                    apply_string_reactive_internal(el, target, string_rx);
                }
                fn apply_pair(rx: silex_core::Rx<Self, silex_core::RxValueKind>, key: String, el: WebElem, target: OwnedApplyTarget) {
                    use silex_core::traits::RxGet;
                    let string_rx = silex_core::Rx::derive(Box::new(move || rx.get().to_string()));
                    apply_string_pair_reactive_internal(el, key, target, string_rx);
                }
                fn into_payload_reactive(rx: silex_core::Rx<Self, silex_core::RxValueKind>) -> Option<AttributePayload> {
                    use silex_core::traits::RxGet;
                    let string_rx = silex_core::Rx::derive(Box::new(move || rx.get().to_string()));
                    Some(AttributePayload::ReactiveString(string_rx))
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
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        let is_class = matches!(target, OwnedApplyTarget::Class)
            || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "class");

        if is_class {
            apply_bool_pair_reactive_internal(el, key, rx);
        }
    }

    fn into_payload_reactive(
        rx: silex_core::Rx<Self, silex_core::RxValueKind>,
    ) -> Option<AttributePayload> {
        Some(AttributePayload::ReactiveBool(rx))
    }
}

// 响应式元组归一化终点：(K, Rx<T>)
impl<K, T> ApplyToDom for (K, silex_core::Rx<T, silex_core::RxValueKind>)
where
    K: AsRef<str> + Clone,
    T: ReactiveApply + Clone + 'static,
{
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        let (key, rx) = self.clone();
        let el = el.clone();
        let owned_target = OwnedApplyTarget::from(target);
        let key_str = key.as_ref().to_string();
        T::apply_pair(rx, key_str, el, owned_target);
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
    let owned_target = OwnedApplyTarget::from(target);
    match owned_target {
        OwnedApplyTarget::Style => {
            if let Some(style) = get_style_decl(el) {
                let _ = style.set_property(key, value);
            }
        }
        OwnedApplyTarget::Attr(ref n) if n == "style" => {
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
                fn apply(&self, el: &WebElem, target: ApplyTarget) {
                    apply_immediate_string(el, target, &self.to_string());
                }

                fn into_payload(self) -> AttributePayload {
                    AttributePayload::StaticString(self.to_string())
                }
            }
        )*
    };
}
impl_apply_to_dom_for_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl<V: ApplyToDom> ApplyToDom for Vec<V> {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        for v in self {
            v.apply(el, target);
        }
    }
}

impl<V: ApplyToDom, const N: usize> ApplyToDom for [V; N] {
    fn apply(&self, el: &WebElem, target: ApplyTarget) {
        for v in self {
            v.apply(el, target);
        }
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
        impl<$($name: ApplyToDom),+> ApplyToDom for AttributeGroup<($($name,)+)> {
            fn apply(&self, el: &WebElem, target: ApplyTarget) {
                #[allow(non_snake_case)]
                let ($($name,)+) = &self.0;
                $($name.apply(el, target);)+
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
pub enum AttributePayload {
    /// 静态字符串：支持预合并（如 class, style）
    StaticString(String),
    /// 静态布尔值：用于开关型属性（如 disabled, checked）
    StaticBool(bool),
    /// 响应式字符串
    ReactiveString(silex_core::Rx<String, silex_core::RxValueKind>),
    /// 响应式布尔值
    ReactiveBool(silex_core::Rx<bool, silex_core::RxValueKind>),
    /// 动态求值载荷
    Dynamic(Rc<dyn Fn(&WebElem, &OwnedApplyTarget)>),
    /// 命令式回调：用于事件监听或复杂的 Mixin 逻辑
    Command(Rc<dyn Fn(&WebElem)>),
}

#[derive(Clone)]
pub struct PendingAttribute {
    pub target: Option<OwnedApplyTarget>,
    pub payload: AttributePayload,
}

impl<'a> From<&'a OwnedApplyTarget> for ApplyTarget<'a> {
    fn from(target: &'a OwnedApplyTarget) -> Self {
        match target {
            OwnedApplyTarget::Attr(n) => ApplyTarget::Attr(n.as_str()),
            OwnedApplyTarget::Prop(n) => ApplyTarget::Prop(n.as_str()),
            OwnedApplyTarget::Class => ApplyTarget::Class,
            OwnedApplyTarget::Style => ApplyTarget::Style,
            OwnedApplyTarget::Apply => ApplyTarget::Apply,
        }
    }
}

pub fn consolidate_attributes(attrs: Vec<PendingAttribute>) -> Vec<PendingAttribute> {
    let mut consolidated = Vec::new();
    let mut classes = Vec::new();
    let mut styles = Vec::new();

    for attr in attrs {
        if let Some(ref target) = attr.target {
            match target {
                OwnedApplyTarget::Class => {
                    if let AttributePayload::StaticString(ref s) = attr.payload {
                        classes.push(s.clone());
                        continue;
                    }
                }
                OwnedApplyTarget::Attr(name) if name == "class" => {
                    if let AttributePayload::StaticString(ref s) = attr.payload {
                        classes.push(s.clone());
                        continue;
                    }
                }
                OwnedApplyTarget::Style => {
                    if let AttributePayload::StaticString(ref s) = attr.payload {
                        styles.push(s.clone());
                        continue;
                    }
                }
                OwnedApplyTarget::Attr(name) if name == "style" => {
                    if let AttributePayload::StaticString(ref s) = attr.payload {
                        styles.push(s.clone());
                        continue;
                    }
                }
                _ => {}
            }
        }
        consolidated.push(attr);
    }

    if !styles.is_empty() {
        let mut merged_style = String::new();
        for s in styles {
            let s = s.trim();
            if !s.is_empty() {
                merged_style.push_str(s);
                if !s.ends_with(';') {
                    merged_style.push(';');
                }
            }
        }
        consolidated.insert(
            0,
            PendingAttribute {
                target: Some(OwnedApplyTarget::Style),
                payload: AttributePayload::StaticString(merged_style),
            },
        );
    }

    if !classes.is_empty() {
        consolidated.insert(
            0,
            PendingAttribute {
                target: Some(OwnedApplyTarget::Class),
                payload: AttributePayload::StaticString(classes.join(" ")),
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
        Self {
            target: Some(target),
            payload: value.into_payload(),
        }
    }

    pub fn apply(&self, el: &WebElem) {
        match &self.payload {
            AttributePayload::StaticString(val) => {
                if let Some(ref t) = self.target {
                    apply_immediate_string(el, ApplyTarget::from(t), val);
                }
            }
            AttributePayload::StaticBool(val) => {
                if let Some(ref t) = self.target {
                    apply_immediate_bool(el, ApplyTarget::from(t), *val);
                }
            }
            AttributePayload::ReactiveString(rx) => {
                if let Some(ref t) = self.target {
                    apply_string_reactive_internal(el.clone(), t.clone(), rx.clone());
                }
            }
            AttributePayload::ReactiveBool(rx) => {
                if let Some(ref t) = self.target {
                    apply_bool_reactive_internal(el.clone(), t.clone(), rx.clone());
                }
            }
            AttributePayload::Dynamic(f) => {
                if let Some(ref t) = self.target {
                    f(el, t);
                }
            }
            AttributePayload::Command(f) => {
                f(el);
            }
        }
    }

    pub fn new_listener(f: impl Fn(&WebElem) + 'static) -> Self {
        Self {
            target: None,
            payload: AttributePayload::Command(Rc::new(f)),
        }
    }
}
