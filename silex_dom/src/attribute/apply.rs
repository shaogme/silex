use silex_core::SilexError;
use silex_core::reactivity::{Constant, Effect, Memo, ReadSignal, RwSignal, Signal};
use silex_core::traits::{Get, IntoRx, RxInternal, With};
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

// --- RxTask: Helper to distinguish different kinds of Rx closures ---

pub trait RxTask<M> {
    fn run_rx(self, el: &WebElem, target: ApplyTarget);
}

// 1. Value Calculation: any RxInternal
impl<F, T> RxTask<silex_core::RxValue> for F
where
    F: RxInternal<Value = T> + 'static,
    T: ReactiveApply + Clone + 'static,
{
    fn run_rx(self, el: &WebElem, target: ApplyTarget) {
        let el = el.clone();
        let owned_target = OwnedApplyTarget::from(target);
        T::apply_to_dom(move || self.try_get().unwrap(), el, owned_target);
    }
}

// 2. Continuous Element Modifier: (&Element) -> ()
impl<F> RxTask<silex_core::RxEffect> for F
where
    F: FnOnce(&WebElem) + 'static,
{
    fn run_rx(self, el: &WebElem, _target: ApplyTarget) {
        // We handle it as a single execution if it's FnOnce.
        // In the future, we could detect if it should be an Effect.
        (self)(el);
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

/// Internal: Set a string attribute or property directly.
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

/// Internal: Shared logic for reactive class updates.
fn create_class_effect_internal(el: WebElem, f: Rc<dyn Fn() -> String>) {
    // Diffing updates
    let prev_classes = Rc::new(RefCell::new(HashSet::new()));

    Effect::new(move |_| {
        let value = f();
        let new_classes: HashSet<String> =
            value.split_whitespace().map(|s| s.to_string()).collect();

        let mut prev = prev_classes.borrow_mut();
        let list = el.class_list();

        // Remove stale classes
        for c in prev.difference(&new_classes) {
            let _ = list.remove_1(c);
        }

        // Add new classes
        for c in new_classes.difference(&prev) {
            let _ = list.add_1(c);
        }

        *prev = new_classes;
    });
}

/// Internal: Shared logic for reactive style updates.
fn create_style_effect_internal(el: WebElem, f: Rc<dyn Fn() -> String>) {
    let prev_keys = Rc::new(RefCell::new(HashSet::<String>::new()));

    Effect::new(move |_| {
        let value = f();
        let new_style_str = value.as_ref();

        if let Some(style) = get_style_decl(&el) {
            let mut prev = prev_keys.borrow_mut();
            let params = parse_style_str(new_style_str);
            let new_keys: HashSet<String> = params.iter().map(|(k, _)| k.clone()).collect();

            // Remove keys that are in prev but not in new
            for k in prev.difference(&new_keys) {
                let _ = style.remove_property(k);
            }

            // Update all current properties
            for (k, v) in params {
                let _ = style.set_property(&k, &v);
            }

            *prev = new_keys;
        }
    });
}

/// Internal: Reactive application of a string value to a target.
fn apply_string_reactive_internal(
    el: WebElem,
    target: OwnedApplyTarget,
    f: Rc<dyn Fn() -> String>,
) {
    match target {
        OwnedApplyTarget::Class => create_class_effect_internal(el, f),
        OwnedApplyTarget::Style => create_style_effect_internal(el, f),
        OwnedApplyTarget::Attr(name) => {
            if name == "class" {
                create_class_effect_internal(el, f);
            } else if name == "style" {
                create_style_effect_internal(el, f);
            } else {
                Effect::new(move |_| {
                    let value = f();
                    set_string_property_internal(&el, &name, &value, false);
                });
            }
        }
        OwnedApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                let value = f();
                set_string_property_internal(&el, &name, &value, true);
            });
        }
        OwnedApplyTarget::Apply => {}
    }
}

fn apply_via_signal<S>(source: S, el: &WebElem, target: ApplyTarget)
where
    S: IntoRx,
    S::Value: ReactiveApply + Clone + 'static,
    S::RxType: With<Value = S::Value> + Clone + 'static,
{
    let signal = source.into_rx();
    let owned_target = OwnedApplyTarget::from(target);
    let el = el.clone();

    S::Value::apply_to_dom(move || signal.with(|v| v.clone()), el, owned_target);
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
    fn apply_to_dom(f: impl Fn() -> Self + 'static, el: WebElem, target: OwnedApplyTarget);

    // New method to handle (Key, Value) pair application
    fn apply_pair(
        f: impl Fn() -> Self + 'static,
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        // Default implementation does nothing
        let _ = (f, key, el, target);
    }
}

// --- Implementations ---

// --- Immediate Application Helpers ---

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

// 1. Static Strings (&str, String, &String)
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

// 2. Bool (Attributes Only)
impl ApplyToDom for bool {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        apply_immediate_bool(el, target, self);
    }
}

// 3. Option
impl<T: ApplyToDom> ApplyToDom for Option<T> {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        if let Some(val) = self {
            val.apply(el, target);
        }
    }
}

// 4. Reactive Closures Implementation (via ReactiveApply)

// 4.1 String Implementation (Reuses diffing logic for Class/Style)
impl ReactiveApply for String {
    fn apply_to_dom(f: impl Fn() -> Self + 'static, el: WebElem, target: OwnedApplyTarget) {
        apply_string_reactive_internal(el, target, Rc::new(f));
    }

    fn apply_pair(
        f: impl Fn() -> Self + 'static,
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        let is_style = matches!(target, OwnedApplyTarget::Style)
            || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "style");

        if is_style && let Some(style) = get_style_decl(&el) {
            Effect::new(move |_| {
                let v = f();
                let _ = style.set_property(&key, &v);
            });
        }
    }
}

// 4.1b &str Implementation
impl ReactiveApply for &str {
    fn apply_to_dom(f: impl Fn() -> Self + 'static, el: WebElem, target: OwnedApplyTarget) {
        apply_string_reactive_internal(el, target, Rc::new(move || f().to_string()))
    }

    fn apply_pair(
        f: impl Fn() -> Self + 'static,
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        String::apply_pair(move || f().to_string(), key, el, target)
    }
}

/// Internal: Shared logic for reactive boolean update.
fn apply_bool_reactive_internal(el: WebElem, target: OwnedApplyTarget, f: Rc<dyn Fn() -> bool>) {
    match target {
        OwnedApplyTarget::Attr(name) => {
            Effect::new(move |_| {
                let val = f();
                if val {
                    let _ = el.set_attribute(&name, "");
                } else {
                    let _ = el.remove_attribute(&name);
                }
            });
        }
        OwnedApplyTarget::Prop(name) => {
            Effect::new(move |_| {
                let val = f();
                let _ =
                    js_sys::Reflect::set(&el, &JsValue::from_str(&name), &JsValue::from_bool(val));
            });
        }
        _ => {}
    }
}

/// Internal: Shared logic for toggling a class reactively.
fn apply_bool_pair_reactive_internal(el: WebElem, key: String, f: Rc<dyn Fn() -> bool>) {
    let list = el.class_list();
    Effect::new(move |_| {
        if f() {
            let _ = list.add_1(&key);
        } else {
            let _ = list.remove_1(&key);
        }
    });
}

// 4.2 Boolean Implementation
impl ReactiveApply for bool {
    fn apply_to_dom(f: impl Fn() -> Self + 'static, el: WebElem, target: OwnedApplyTarget) {
        apply_bool_reactive_internal(el, target, Rc::new(f));
    }

    fn apply_pair(
        f: impl Fn() -> Self + 'static,
        key: String,
        el: WebElem,
        target: OwnedApplyTarget,
    ) {
        let is_class = matches!(target, OwnedApplyTarget::Class)
            || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "class");

        if is_class {
            apply_bool_pair_reactive_internal(el, key, Rc::new(f));
        }
    }
}

macro_rules! impl_reactive_apply_primitive {
    ($($t:ty),*) => {
        $(
            impl ReactiveApply for $t {
                fn apply_to_dom(f: impl Fn() -> Self + 'static, el: WebElem, target: OwnedApplyTarget) {
                    apply_string_reactive_internal(el, target, Rc::new(move || f().to_string()));
                }
                fn apply_pair(f: impl Fn() -> Self + 'static, key: String, el: WebElem, target: OwnedApplyTarget) {
                    let is_style = matches!(target, OwnedApplyTarget::Style)
                        || matches!(target, OwnedApplyTarget::Attr(ref n) if n == "style");

                    if is_style && let Some(style) = get_style_decl(&el) {
                        Effect::new(move |_| {
                            let _ = style.set_property(&key, &f().to_string());
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

// 4.3 Blanket Implementation for Rx
impl<F, M> ApplyToDom for silex_core::Rx<F, M>
where
    F: RxTask<M>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        self.0.run_rx(el, target);
    }
}

// 5. Signals

macro_rules! impl_apply_basic_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> ApplyToDom for $ty<T>
            where
                T: ReactiveApply + Clone + 'static,
            {
                fn apply(self, el: &WebElem, target: ApplyTarget) {
                    apply_via_signal::<Self>(self, el, target);
                }
            }
        )*
    }
}

impl_apply_basic_signal!(ReadSignal, RwSignal, Signal, Constant);

impl<T> ApplyToDom for Memo<T>
where
    T: ReactiveApply + Clone + PartialEq + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        apply_via_signal::<Self>(self, el, target);
    }
}

// 6 Generic Tuple Implementation ((Key, Value))
impl<K, S> ApplyToDom for (K, S)
where
    K: AsRef<str>,
    S: IntoRx,
    S::Value: ReactiveApply + Clone + 'static,
    S::RxType: With<Value = S::Value> + Clone + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let (key, source) = self;
        let signal = source.into_rx();
        let el = el.clone();
        let owned_target = OwnedApplyTarget::from(target);
        let key_str = key.as_ref().to_string();

        S::Value::apply_pair(
            move || signal.with(|v| v.clone()),
            key_str,
            el,
            owned_target,
        );
    }
}

// Primitives directly
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

// 7. Collections
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

/// Stores an attribute application operation to be executed later.
/// Used for forwarding attributes from Components to their root elements.
#[derive(Clone)]
pub struct PendingAttribute {
    f: Rc<dyn Fn(&WebElem)>,
}

impl PendingAttribute {
    /// Build a pending attribute that will be applied later.
    /// Uses take-out semantics to avoid Clone requirement - the value is consumed on first apply.
    pub fn build<V>(value: V, target: OwnedApplyTarget) -> Self
    where
        V: ApplyToDom + 'static,
    {
        // Wrap value in Rc<RefCell<Option<V>>> for take-out pattern
        let value_cell = Rc::new(RefCell::new(Some(value)));

        Self {
            f: Rc::new(move |el| {
                // Take the value out (only succeeds once)
                if let Some(v) = value_cell.borrow_mut().take() {
                    // Map OwnedApplyTarget to ApplyTarget
                    let t = match &target {
                        OwnedApplyTarget::Attr(n) => ApplyTarget::Attr(n),
                        OwnedApplyTarget::Prop(n) => ApplyTarget::Prop(n),
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
