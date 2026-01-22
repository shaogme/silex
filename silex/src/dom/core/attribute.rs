use crate::SilexError;
use crate::reactivity::{ReadSignal, RwSignal, create_effect};
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
        if let Err(e) = el.set_attribute(name, self).map_err(SilexError::from) {
            crate::error::handle_error(e);
        }
    }
}

impl AttributeValue for String {
    fn apply(self, el: &WebElem, name: &str) {
        if let Err(e) = el.set_attribute(name, &self).map_err(SilexError::from) {
            crate::error::handle_error(e);
        }
    }
}

impl AttributeValue for &String {
    fn apply(self, el: &WebElem, name: &str) {
        if let Err(e) = el.set_attribute(name, &self).map_err(SilexError::from) {
            crate::error::handle_error(e);
        }
    }
}

impl AttributeValue for bool {
    fn apply(self, el: &WebElem, name: &str) {
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
        // 自动创建副作用
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

// 3. 直接 Signal 支持
impl<T> AttributeValue for ReadSignal<T>
where
    T: AsRef<str> + Clone + 'static,
{
    fn apply(self, el: &WebElem, name: &str) {
        let el = el.clone();
        let name = name.to_string();
        // Signal 是 Copy 的，直接移动进去
        let signal = self;
        create_effect(move || {
            let v = signal.get();
            if let Err(e) = el
                .set_attribute(&name, v.as_ref())
                .map_err(SilexError::from)
            {
                crate::error::handle_error(e);
            }
        });
    }
}

impl<T> AttributeValue for RwSignal<T>
where
    T: AsRef<str> + Clone + 'static,
{
    fn apply(self, el: &WebElem, name: &str) {
        self.read_signal().apply(el, name);
    }
}

// 4. 样式对象/Map 支持: .style(("color", "red"))
// 使用具体的 String/&str 实现以避免与 (K, bool) 和 (K, F) 冲突

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
            let class_name = self.0.as_ref();
            let is_active = self.1;
            let list = el.class_list();
            if is_active {
                let _ = list.add_1(class_name);
            } else {
                let _ = list.remove_1(class_name);
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
            let class_name = self.0.as_ref().to_string();
            create_effect(move || {
                let is_active = self.1();
                let list = el.class_list();
                if is_active {
                    let _ = list.add_1(&class_name);
                } else {
                    let _ = list.remove_1(&class_name);
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
