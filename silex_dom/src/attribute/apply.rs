use silex_core::SilexError;
use silex_core::reactivity::{Effect, Signal};
use silex_core::traits::{IntoSignal, RxGet};
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
    fn apply(self, el: &WebElem, target: ApplyTarget);
}

impl<F> ApplyToDom for F
where
    F: FnOnce(&WebElem) + 'static,
{
    fn apply(self, el: &WebElem, _target: ApplyTarget) {
        (self)(el);
    }
}

// --- 统一响应式应用逻辑 ---

// 1. 已归一化的 Rx 容器 (Value)
impl<F> ApplyToDom for silex_core::Rx<F, silex_core::RxValueKind>
where
    Self: silex_core::traits::IntoSignal + 'static,
    <Self as silex_core::traits::RxValue>::Value: ReactiveApply + Clone + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        crate::attribute::apply_signal_internal(self.into_signal(), el, target);
    }
}

// 2. 逻辑型 Rx (Effect)
impl<F> ApplyToDom for silex_core::Rx<F, silex_core::RxEffectKind>
where
    F: FnOnce(&WebElem) + 'static,
{
    fn apply(self, el: &WebElem, _target: ApplyTarget) {
        (self.0)(el);
    }
}

// 2. 响应式原语转发器 (不再依赖 IntoStorable 的阶段转换)
macro_rules! impl_apply_to_dom_rx_forwarder {
    ($($ty:ident),*) => {
        $(
            impl<T> ApplyToDom for silex_core::reactivity::$ty<T>
            where
                T: ReactiveApply + Clone + 'static,
                Self: silex_core::traits::IntoRx + 'static,
                <Self as silex_core::traits::IntoRx>::RxType: ApplyToDom,
            {
                fn apply(self, el: &WebElem, target: ApplyTarget) {
                    use silex_core::traits::IntoRx;
                    self.into_rx().apply(el, target);
                }
            }
        )*
    };
}

impl_apply_to_dom_rx_forwarder!(Signal, ReadSignal, RwSignal, Constant, Memo);

// 3. 响应式组合/派生类型转发器
impl<S, F> ApplyToDom for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyToDom,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        use silex_core::traits::IntoRx;
        self.into_rx().apply(el, target);
    }
}

impl<U, const N: usize> ApplyToDom for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyToDom,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        use silex_core::traits::IntoRx;
        self.into_rx().apply(el, target);
    }
}

impl<S, F, O> ApplyToDom for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyToDom,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        use silex_core::traits::IntoRx;
        self.into_rx().apply(el, target);
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

fn create_class_effect_internal(el: WebElem, signal: Signal<String>) {
    let prev_classes = Rc::new(RefCell::new(HashSet::new()));

    Effect::new(move |_| {
        let value = signal.get();
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

fn create_style_effect_internal(el: WebElem, signal: Signal<String>) {
    let prev_keys = Rc::new(RefCell::new(HashSet::<String>::new()));

    Effect::new(move |_| {
        let value = signal.get();
        let new_style_str = value.as_ref();

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

fn apply_string_reactive_internal(el: WebElem, target: OwnedApplyTarget, signal: Signal<String>) {
    match target {
        OwnedApplyTarget::Class => create_class_effect_internal(el, signal),
        OwnedApplyTarget::Style => create_style_effect_internal(el, signal),
        OwnedApplyTarget::Attr(name) => {
            if name == "class" {
                create_class_effect_internal(el, signal);
            } else if name == "style" {
                create_style_effect_internal(el, signal);
            } else {
                Effect::new(move |_| {
                    let value = signal.get();
                    set_string_property_internal(&el, &name, &value, false);
                });
            }
        }
        OwnedApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                let value = signal.get();
                set_string_property_internal(&el, &name, &value, true);
            });
        }
        OwnedApplyTarget::Apply => {}
    }
}

fn apply_bool_reactive_internal(el: WebElem, target: OwnedApplyTarget, signal: Signal<bool>) {
    match target {
        OwnedApplyTarget::Attr(name) => {
            Effect::new(move |_| {
                let val = signal.get();
                if val {
                    let _ = el.set_attribute(&name, "");
                } else {
                    let _ = el.remove_attribute(&name);
                }
            });
        }
        OwnedApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                let val = signal.get();
                let _ =
                    js_sys::Reflect::set(&el, &JsValue::from_str(&name), &JsValue::from_bool(val));
            });
        }
        _ => {}
    }
}

fn apply_bool_pair_reactive_internal(el: WebElem, key: String, signal: Signal<bool>) {
    let list = el.class_list();
    Effect::new(move |_| {
        if signal.get() {
            let _ = list.add_1(&key);
        } else {
            let _ = list.remove_1(&key);
        }
    });
}

pub(crate) fn apply_signal_internal<T>(signal: Signal<T>, el: &WebElem, target: ApplyTarget)
where
    T: ReactiveApply + Clone + 'static,
{
    let owned_target = OwnedApplyTarget::from(target);
    T::apply_to_dom(signal, el.clone(), owned_target);
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
    fn apply_to_dom(signal: Signal<Self>, el: WebElem, target: OwnedApplyTarget)
    where
        Self: Sized;

    fn apply_pair(signal: Signal<Self>, key: String, el: WebElem, target: OwnedApplyTarget)
    where
        Self: Sized,
    {
        let _ = (signal, key, el, target);
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
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }
}

impl ApplyToDom for String {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, &self);
    }
}

impl ApplyToDom for &String {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_string(el, target, self);
    }
}

impl ApplyToDom for bool {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_bool(el, target, self);
    }
}

impl<T: ApplyToDom> ApplyToDom for Option<T> {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        if let Some(val) = self {
            val.apply(el, target);
        }
    }
}

impl ReactiveApply for String {
    fn apply_to_dom(signal: Signal<Self>, el: WebElem, target: OwnedApplyTarget) {
        apply_string_reactive_internal(el, target, signal);
    }

    fn apply_pair(signal: Signal<Self>, key: String, el: WebElem, target: OwnedApplyTarget) {
        let is_style = matches!(target, OwnedApplyTarget::Style)
            || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "style");

        if is_style && let Some(style) = get_style_decl(&el) {
            Effect::new(move |_| {
                let v = signal.get();
                let _ = style.set_property(&key, &v);
            });
        }
    }
}

impl ReactiveApply for &'static str {
    fn apply_to_dom(signal: Signal<Self>, el: WebElem, target: OwnedApplyTarget) {
        let string_signal =
            silex_core::reactivity::Signal::derive(move || signal.get().to_string());
        apply_string_reactive_internal(el, target, string_signal);
    }

    fn apply_pair(signal: Signal<Self>, key: String, el: WebElem, target: OwnedApplyTarget) {
        let string_signal =
            silex_core::reactivity::Signal::derive(move || signal.get().to_string());
        String::apply_pair(string_signal, key, el, target)
    }
}

macro_rules! impl_reactive_apply_primitive {
    ($($t:ty),*) => {
        $(
            impl ReactiveApply for $t {
                fn apply_to_dom(signal: Signal<Self>, el: WebElem, target: OwnedApplyTarget) {
                    let string_signal = silex_core::reactivity::Signal::derive(move || signal.get().to_string());
                    apply_string_reactive_internal(el, target, string_signal);
                }
                fn apply_pair(signal: Signal<Self>, key: String, el: WebElem, target: OwnedApplyTarget) {
                    let is_style = matches!(target, OwnedApplyTarget::Style)
                        || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "style");

                    if is_style && let Some(style) = get_style_decl(&el) {
                        Effect::new(move |_| {
                            let _ = style.set_property(&key, &signal.get().to_string());
                        });
                    }
                }
            }
        )*
    };
}

impl_reactive_apply_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl ReactiveApply for bool {
    fn apply_to_dom(signal: Signal<Self>, el: WebElem, target: OwnedApplyTarget) {
        apply_bool_reactive_internal(el, target, signal);
    }

    fn apply_pair(signal: Signal<Self>, key: String, el: WebElem, target: OwnedApplyTarget) {
        let is_class = matches!(target, OwnedApplyTarget::Class)
            || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "class");

        if is_class {
            apply_bool_pair_reactive_internal(el, key, signal);
        }
    }
}

// 响应式元组：(Key, ReactiveSource)
macro_rules! impl_apply_to_dom_tuple_forwarder {
    ($($ty:ident),*) => {
        $(
            impl<K, T> ApplyToDom for (K, silex_core::reactivity::$ty<T>)
            where
                K: AsRef<str>,
                T: ReactiveApply + Clone + 'static,
            {
                fn apply(self, el: &WebElem, target: ApplyTarget) {
                    let (key, source) = self;
                    let signal = source.into_signal();
                    let el = el.clone();
                    let owned_target = OwnedApplyTarget::from(target);
                    let key_str = key.as_ref().to_string();
                    T::apply_pair(signal, key_str, el, owned_target);
                }
            }
        )*
    };
}

impl_apply_to_dom_tuple_forwarder!(Signal, ReadSignal, RwSignal, Constant, Memo);

impl<K, F, M> ApplyToDom for (K, silex_core::Rx<F, M>)
where
    K: AsRef<str>,
    silex_core::Rx<F, M>: silex_core::traits::IntoSignal + 'static,
    <silex_core::Rx<F, M> as silex_core::traits::RxValue>::Value: ReactiveApply + Clone + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let (key, source) = self;
        let signal = source.into_signal();
        let el = el.clone();
        let owned_target = OwnedApplyTarget::from(target);
        let key_str = key.as_ref().to_string();
        <<silex_core::Rx<F, M> as silex_core::traits::RxValue>::Value>::apply_pair(
            signal,
            key_str,
            el,
            owned_target,
        );
    }
}

// 静态元组 (Key, StaticValue)
impl<K> ApplyToDom for (K, String)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let (key, value) = self;
        apply_static_pair(el, target, key.as_ref(), &value);
    }
}

impl<K> ApplyToDom for (K, &str)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let (key, value) = self;
        apply_static_pair(el, target, key.as_ref(), value);
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
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let (key, value) = self;
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
                fn apply(self, el: &WebElem, target: ApplyTarget) {
                    apply_immediate_string(el, target, &self.to_string());
                }
            }
        )*
    };
}
impl_apply_to_dom_for_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl<V: ApplyToDom> ApplyToDom for Vec<V> {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        for v in self {
            v.apply(el, target);
        }
    }
}

impl<V: ApplyToDom, const N: usize> ApplyToDom for [V; N] {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
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
            fn apply(self, el: &WebElem, target: ApplyTarget) {
                #[allow(non_snake_case)]
                let ($($name,)+) = self.0;
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
pub struct PendingAttribute {
    f: Rc<dyn Fn(&WebElem)>,
}

impl PendingAttribute {
    pub fn build<V>(value: V, target: OwnedApplyTarget) -> Self
    where
        V: ApplyToDom + 'static,
    {
        let value_cell = Rc::new(RefCell::new(Some(value)));

        Self {
            f: Rc::new(move |el| {
                if let Some(v) = value_cell.borrow_mut().take() {
                    let t = match &target {
                        OwnedApplyTarget::Attr(n) => ApplyTarget::Attr(n.as_str()),
                        OwnedApplyTarget::Prop(n) => ApplyTarget::Prop(n.as_str()),
                        OwnedApplyTarget::Class => ApplyTarget::Class,
                        OwnedApplyTarget::Style => ApplyTarget::Style,
                        OwnedApplyTarget::Apply => ApplyTarget::Apply,
                    };
                    v.apply(el, t);
                }
            }),
        }
    }

    pub fn apply(&self, el: &WebElem) {
        (self.f)(el);
    }

    pub fn new_listener(f: impl Fn(&WebElem) + 'static) -> Self {
        Self { f: Rc::new(f) }
    }
}
