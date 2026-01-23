use crate::SilexError;
use crate::reactivity::{ReadSignal, RwSignal, create_effect};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::Element as WebElem;

// --- 核心魔法：统一的应用目标枚举 ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyTarget<'a> {
    /// 标准属性，如 `id`, `href`, `src`。也包含作为属性调用的 `class` 和 `style`。
    Attr(&'a str),
    /// 专门的 `.class(...)` 调用
    Class,
    /// 专门的 `.style(...)` 调用
    Style,
}

/// 任何可以用作 HTML 属性、类名或样式的类型
/// 这是对 AttributeValue, ApplyClass, ApplyStyle 的统一替代
pub trait ApplyToDom {
    fn apply(self, el: &WebElem, target: ApplyTarget);
}

// --- Helper Functions (Private) ---

fn handle_err(res: Result<(), SilexError>) {
    if let Err(e) = res {
        crate::error::handle_error(e);
    }
}

fn apply_class_static(el: &WebElem, val: &str) {
    let list = el.class_list();
    for c in val.split_whitespace() {
        handle_err(list.add_1(c).map_err(SilexError::from));
    }
}

fn get_style_decl(el: &WebElem) -> Option<web_sys::CssStyleDeclaration> {
    if let Some(e) = el.dyn_ref::<web_sys::HtmlElement>() {
        Some(e.style())
    } else if let Some(e) = el.dyn_ref::<web_sys::SvgElement>() {
        Some(e.style())
    } else {
        None
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

fn apply_style_static(el: &WebElem, val: &str) {
    if let Some(style) = get_style_decl(el) {
        for (k, v) in parse_style_str(val) {
            let _ = style.set_property(&k, &v);
        }
    }
}

fn apply_style_kv(el: &WebElem, k: &str, v: &str) {
    if let Some(style) = get_style_decl(el) {
        let _ = style.set_property(k, v);
    }
}

fn create_class_effect<F, S>(el: WebElem, f: F)
where
    F: Fn() -> S + 'static,
    S: AsRef<str>,
{
    // Diffing updates
    let prev_classes = Rc::new(RefCell::new(HashSet::new()));

    create_effect(move || {
        let value = f();
        let new_classes: HashSet<String> = value
            .as_ref()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

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

fn create_style_effect<F, S>(el: WebElem, f: F)
where
    F: Fn() -> S + 'static,
    S: AsRef<str>,
{
    let prev_keys = Rc::new(RefCell::new(HashSet::<String>::new()));

    create_effect(move || {
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

// --- Implementations ---

// 1. Static Strings (&str, String, &String)
impl ApplyToDom for &str {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        match target {
            ApplyTarget::Class => apply_class_static(el, self),
            ApplyTarget::Style => apply_style_static(el, self),
            ApplyTarget::Attr(name) => {
                if name == "class" {
                    apply_class_static(el, self);
                } else if name == "style" {
                    apply_style_static(el, self);
                } else {
                    handle_err(el.set_attribute(name, self).map_err(SilexError::from));
                }
            }
        }
    }
}

impl ApplyToDom for String {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        self.as_str().apply(el, target)
    }
}

impl ApplyToDom for &String {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        self.as_str().apply(el, target)
    }
}

// 2. Bool (Attributes Only)
impl ApplyToDom for bool {
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        if let ApplyTarget::Attr(name) = target {
            // Boolean attributes (e.g. checked, disabled)
            let res = if self {
                el.set_attribute(name, "").map_err(SilexError::from)
            } else {
                el.remove_attribute(name).map_err(SilexError::from)
            };
            handle_err(res);
        }
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

// 4. Reactive Closures
impl<F, S> ApplyToDom for F
where
    F: Fn() -> S + 'static,
    S: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let el = el.clone();

        match target {
            ApplyTarget::Class => create_class_effect(el, self),
            ApplyTarget::Style => create_style_effect(el, self),
            ApplyTarget::Attr(name) => {
                let name = name.to_string(); // Capture for 'static closure

                if name == "class" {
                    create_class_effect(el, self);
                } else if name == "style" {
                    create_style_effect(el, self);
                } else {
                    create_effect(move || {
                        let value = self();
                        handle_err(
                            el.set_attribute(&name, value.as_ref())
                                .map_err(SilexError::from),
                        );
                    });
                }
            }
        }
    }
}

// 5. Signals
impl<T> ApplyToDom for ReadSignal<T>
where
    T: std::fmt::Display + Clone + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        (move || self.get().to_string()).apply(el, target);
    }
}

impl<T> ApplyToDom for RwSignal<T>
where
    T: std::fmt::Display + Clone + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        (move || self.get().to_string()).apply(el, target);
    }
}

// 6. Tuples

// Helper macro to avoid creating overlapping implementations
macro_rules! impl_tuple_kv_str {
    ($key:ty, $val:ty) => {
        impl ApplyToDom for ($key, $val) {
            fn apply(self, el: &WebElem, target: ApplyTarget) {
                // Key-Value Style pair
                let is_style = match target {
                    ApplyTarget::Style => true,
                    ApplyTarget::Attr(n) if n == "style" => true,
                    _ => false,
                };
                if is_style {
                    apply_style_kv(el, self.0.as_ref(), self.1.as_ref());
                }
            }
        }
    };
}

// Distinct implementations for common string types to avoid overlap with bool
impl_tuple_kv_str!(&str, &str);
impl_tuple_kv_str!(&str, String);
impl_tuple_kv_str!(&str, &String);
impl_tuple_kv_str!(String, &str);
impl_tuple_kv_str!(String, String);
impl_tuple_kv_str!(String, &String);

// 6.2 (Key, bool) for Conditional Class
impl<K> ApplyToDom for (K, bool)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let is_class = match target {
            ApplyTarget::Class => true,
            ApplyTarget::Attr(n) if n == "class" => true,
            _ => false,
        };

        if is_class {
            let class_names = self.0.as_ref();
            let is_active = self.1;
            let list = el.class_list();
            for c in class_names.split_whitespace() {
                if is_active {
                    let _ = list.add_1(c);
                } else {
                    let _ = list.remove_1(c);
                }
            }
        }
    }
}

// 6.3 (Key, Fn -> bool) for Reactive Conditional Class
impl<K, F> ApplyToDom for (K, F)
where
    K: AsRef<str>,
    F: Fn() -> bool + 'static,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        let is_class = match target {
            ApplyTarget::Class => true,
            ApplyTarget::Attr(n) if n == "class" => true,
            _ => false,
        };

        if is_class {
            let el = el.clone();
            let raw_class_names = self.0.as_ref().to_string();
            create_effect(move || {
                let is_active = self.1();
                let list = el.class_list();
                for c in raw_class_names.split_whitespace() {
                    if is_active {
                        let _ = list.add_1(c);
                    } else {
                        let _ = list.remove_1(c);
                    }
                }
            });
        }
    }
}

// 6.4 (Key, Signal<bool>) -> Delegate to Closure
impl<K> ApplyToDom for (K, ReadSignal<bool>)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        (self.0, move || self.1.get()).apply(el, target);
    }
}

impl<K> ApplyToDom for (K, RwSignal<bool>)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, target: ApplyTarget) {
        (self.0, move || self.1.get()).apply(el, target);
    }
}

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
