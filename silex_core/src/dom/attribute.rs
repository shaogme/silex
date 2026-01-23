use crate::SilexError;
use crate::reactivity::{ReadSignal, RwSignal, create_effect};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::Element as WebElem;

// --- 核心魔法：多态属性特征 ---

/// 任何可以用作 HTML 属性值的类型
pub trait AttributeValue {
    fn apply(self, el: &WebElem, name: &str);
}

// 1. 静态字符串支持
impl AttributeValue for &str {
    fn apply(self, el: &WebElem, name: &str) {
        if name == "class" {
            let list = el.class_list();
            for c in self.split_whitespace() {
                if let Err(e) = list.add_1(c).map_err(SilexError::from) {
                    crate::error::handle_error(e);
                }
            }
        } else {
            if let Err(e) = el.set_attribute(name, self).map_err(SilexError::from) {
                crate::error::handle_error(e);
            }
        }
    }
}

impl AttributeValue for String {
    fn apply(self, el: &WebElem, name: &str) {
        self.as_str().apply(el, name)
    }
}

impl AttributeValue for &String {
    fn apply(self, el: &WebElem, name: &str) {
        self.as_str().apply(el, name)
    }
}

impl AttributeValue for bool {
    fn apply(self, el: &WebElem, name: &str) {
        // Boolean attributes (e.g. checked, disabled)
        // Note: Generally irrelevant for "class", but we keep standard behavior.
        let res = if self {
            el.set_attribute(name, "").map_err(SilexError::from)
        } else {
            el.remove_attribute(name).map_err(SilexError::from)
        };
        if let Err(e) = res {
            crate::error::handle_error(e);
        }
    }
}

// 2. 动态闭包支持 (Reactive Closure)
impl<F, S> AttributeValue for F
where
    F: Fn() -> S + 'static,
    S: AsRef<str>,
{
    fn apply(self, el: &WebElem, name: &str) {
        let el = el.clone();
        let name = name.to_string();

        if name == "class" {
            // Class specific logic: Diffing updates
            let prev_classes = Rc::new(RefCell::new(HashSet::new()));

            create_effect(move || {
                let value = self();
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
        } else {
            // Standard attribute: set_attribute replacement
            create_effect(move || {
                let value = self();
                if let Err(e) = el
                    .set_attribute(&name, value.as_ref())
                    .map_err(SilexError::from)
                {
                    crate::error::handle_error(e);
                }
            });
        }
    }
}

// 3. 直接 Signal 支持
impl<T> AttributeValue for ReadSignal<T>
where
    T: std::fmt::Display + Clone + 'static,
{
    fn apply(self, el: &WebElem, name: &str) {
        // Delegate to closure implementation
        (move || self.get().to_string()).apply(el, name);
    }
}

impl<T> AttributeValue for RwSignal<T>
where
    T: std::fmt::Display + Clone + 'static,
{
    fn apply(self, el: &WebElem, name: &str) {
        (move || self.get().to_string()).apply(el, name);
    }
}

// 4. 样式对象/Map 支持: .style(("color", "red"))

// Case 4.1: (K, &str)
impl<K> AttributeValue for (K, &str)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, name: &str) {
        if name == "style" {
            // style 属性特殊处理
            if let Some(target) = el.dyn_ref::<web_sys::HtmlElement>() {
                let _ = target.style().set_property(self.0.as_ref(), self.1);
            } else if let Some(target) = el.dyn_ref::<web_sys::SvgElement>() {
                let _ = target.style().set_property(self.0.as_ref(), self.1);
            }
        }
    }
}

// Case 4.2: (K, String)
impl<K> AttributeValue for (K, String)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, name: &str) {
        if name == "style" {
            if let Some(target) = el.dyn_ref::<web_sys::HtmlElement>() {
                let _ = target.style().set_property(self.0.as_ref(), &self.1);
            } else if let Some(target) = el.dyn_ref::<web_sys::SvgElement>() {
                let _ = target.style().set_property(self.0.as_ref(), &self.1);
            }
        }
    }
}

// 5. 动态类名支持 (Tuple): .class("active", bool)
impl<K> AttributeValue for (K, bool)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, name: &str) {
        if name == "class" {
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

// 6. 动态类名支持 (Closure): .class("active", || signal.get())
impl<K, F> AttributeValue for (K, F)
where
    K: AsRef<str>,
    F: Fn() -> bool + 'static,
{
    fn apply(self, el: &WebElem, name: &str) {
        if name == "class" {
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

// 7. 动态类名支持 (Signal): .class("active", signal)
impl<K> AttributeValue for (K, ReadSignal<bool>)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, name: &str) {
        // 复用 Closure 逻辑
        (self.0, move || self.1.get()).apply(el, name);
    }
}

impl<K> AttributeValue for (K, RwSignal<bool>)
where
    K: AsRef<str>,
{
    fn apply(self, el: &WebElem, name: &str) {
        (self.0, move || self.1.get()).apply(el, name);
    }
}

// 8. 集合支持: Vec, Array
impl<V: AttributeValue> AttributeValue for Vec<V> {
    fn apply(self, el: &WebElem, name: &str) {
        for v in self {
            v.apply(el, name);
        }
    }
}

impl<V: AttributeValue, const N: usize> AttributeValue for [V; N] {
    fn apply(self, el: &WebElem, name: &str) {
        for v in self {
            v.apply(el, name);
        }
    }
}
