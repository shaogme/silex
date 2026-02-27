use silex_core::Rx;
use silex_core::reactivity::Effect;
use silex_core::traits::RxGet;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Element, HtmlElement, SvgElement};

#[derive(Clone)]
pub enum AttrOp {
    // --- 通用属性与 Property ---
    SetStaticAttr {
        name: Cow<'static, str>,
        value: Cow<'static, str>,
    },
    SetStaticProp {
        name: Cow<'static, str>,
        value: JsValue,
    },
    BindReactiveAttr {
        name: Cow<'static, str>,
        rx: Rx<String>,
    },
    BindReactiveProp {
        name: Cow<'static, str>,
        rx: Rx<JsValue>,
    },

    // 新增：布尔值属性与 Property
    SetStaticBoolAttr {
        name: Cow<'static, str>,
        value: bool,
    },
    SetStaticBoolProp {
        name: Cow<'static, str>,
        value: bool,
    },
    BindReactiveBoolAttr {
        name: Cow<'static, str>,
        rx: Rx<bool>,
    },
    BindReactiveBoolProp {
        name: Cow<'static, str>,
        rx: Rx<bool>,
    },

    // --- Class 专项优化（收敛意图） ---
    SetStaticClasses(Vec<Cow<'static, str>>),
    AddClassToggle {
        name: Cow<'static, str>,
        rx: Rx<bool>,
    },
    AddReactiveClasses(Rx<String>),

    // --- Style 专项优化（收敛意图） ---
    SetStaticStyles(Vec<(Cow<'static, str>, Cow<'static, str>)>),
    BindStyleProperty {
        name: Cow<'static, str>,
        rx: Rx<String>,
    },
    BindReactiveStyleSheet(Rx<String>),

    // --- 阶段三：单 Effect 策略优化 (全面转向 AttrOp 的核心) ---
    CombinedClasses {
        statics: Vec<Cow<'static, str>>,
        toggles: Vec<(Cow<'static, str>, Rx<bool>)>,
        reactives: Vec<Rx<String>>,
    },
    CombinedStyles {
        statics: Vec<(Cow<'static, str>, Cow<'static, str>)>,
        properties: Vec<(Cow<'static, str>, Rx<String>)>,
        sheets: Vec<Rx<String>>,
    },

    // --- 逃逸舱与特殊指令 ---
    Custom(Rc<dyn Fn(&Element)>),
    Noop,
}

impl AttrOp {
    pub fn apply(self, el: &Element) {
        match self {
            AttrOp::SetStaticAttr { name, value } => {
                set_string_property_internal(el, &name, &value, false);
            }
            AttrOp::SetStaticProp { name, value } => {
                let _ = js_sys::Reflect::set(el, &JsValue::from_str(&name), &value);
            }
            AttrOp::BindReactiveAttr { name, rx } => {
                let el = el.clone();
                Effect::new(move |_| {
                    set_string_property_internal(&el, &name, &rx.get(), false);
                });
            }
            AttrOp::BindReactiveProp { name, rx } => {
                let el = el.clone();
                Effect::new(move |_| {
                    let _ = js_sys::Reflect::set(&el, &JsValue::from_str(&name), &rx.get());
                });
            }
            AttrOp::SetStaticBoolAttr { name, value } => {
                apply_immediate_bool_internal(el, &name, value, false);
            }
            AttrOp::SetStaticBoolProp { name, value } => {
                apply_immediate_bool_internal(el, &name, value, true);
            }
            AttrOp::BindReactiveBoolAttr { name, rx } => {
                let el = el.clone();
                Effect::new(move |_| {
                    apply_immediate_bool_internal(&el, &name, rx.get(), false);
                });
            }
            AttrOp::BindReactiveBoolProp { name, rx } => {
                let el = el.clone();
                Effect::new(move |_| {
                    apply_immediate_bool_internal(&el, &name, rx.get(), true);
                });
            }
            AttrOp::SetStaticClasses(classes) => {
                let list = el.class_list();
                for c in classes {
                    let _ = list.add_1(&c);
                }
            }
            AttrOp::AddClassToggle { name, rx } => {
                let list = el.class_list();
                Effect::new(move |_| {
                    if rx.get() {
                        let _ = list.add_1(&name);
                    } else {
                        let _ = list.remove_1(&name);
                    }
                });
            }
            AttrOp::AddReactiveClasses(rx) => {
                let prev_classes = Rc::new(RefCell::new(HashSet::new()));
                let list = el.class_list();
                Effect::new(move |_| {
                    let value = rx.get();
                    let new_classes: HashSet<String> =
                        value.split_whitespace().map(|s| s.to_string()).collect();
                    let mut prev = prev_classes.borrow_mut();

                    for c in prev.difference(&new_classes) {
                        let _ = list.remove_1(c);
                    }
                    for c in new_classes.difference(&prev) {
                        let _ = list.add_1(c);
                    }
                    *prev = new_classes;
                });
            }
            AttrOp::SetStaticStyles(styles) => {
                if let Some(style) = get_style_decl(el) {
                    for (k, v) in styles {
                        let _ = style.set_property(&k, &v);
                    }
                }
            }
            AttrOp::BindStyleProperty { name, rx } => {
                if let Some(style) = get_style_decl(el) {
                    Effect::new(move |_| {
                        let _ = style.set_property(&name, &rx.get());
                    });
                }
            }
            AttrOp::BindReactiveStyleSheet(rx) => {
                let prev_keys = Rc::new(RefCell::new(HashSet::<String>::new()));
                let el = el.clone();
                Effect::new(move |_| {
                    let value = rx.get();
                    if let Some(style) = get_style_decl(&el) {
                        let mut prev = prev_keys.borrow_mut();
                        let params = parse_style_str(&value);
                        let new_keys: HashSet<String> =
                            params.iter().map(|(k, _)| k.to_string()).collect();

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
            AttrOp::Custom(f) => {
                f(el);
            }
            AttrOp::Noop => {}

            // --- 阶段三：合并应用的深度优化 (分发到 Kernel 函数) ---
            AttrOp::CombinedClasses {
                statics,
                toggles,
                reactives,
            } => {
                apply_combined_classes_internal(el, statics, toggles, reactives);
            }
            AttrOp::CombinedStyles {
                statics,
                properties,
                sheets,
            } => {
                apply_combined_styles_internal(el, statics, properties, sheets);
            }
        }
    }
}

// --- Kernel Implementation Functions for Combined Op ---

fn apply_combined_classes_internal(
    el: &Element,
    statics: Vec<Cow<'static, str>>,
    toggles: Vec<(Cow<'static, str>, Rx<bool>)>,
    reactives: Vec<Rx<String>>,
) {
    let list = el.class_list();
    // 1. 立即应用所有静态类（非响应式，仅执行一次）
    for s in &statics {
        let _ = list.add_1(s);
    }

    if toggles.is_empty() && reactives.is_empty() {
        return;
    }

    // 2. 建立单 Effect 追踪所有响应式部分
    let prev_reactive_tokens = Rc::new(RefCell::new(HashSet::<String>::new()));
    let el_clone = el.clone();

    Effect::new(move |_| {
        let list = el_clone.class_list();

        // 处理所有 Toggle (如 .class_toggle)
        for (name, rx) in &toggles {
            if rx.get() {
                let _ = list.add_1(name);
            } else {
                let _ = list.remove_1(name);
            }
        }

        // 处理所有响应式字符串类 (需要 Diff 算法以支持正确删除旧类)
        if !reactives.is_empty() {
            let mut new_tokens = HashSet::new();
            for rx in &reactives {
                for token in rx.get().split_whitespace() {
                    new_tokens.insert(token.to_string());
                }
            }

            let mut prev = prev_reactive_tokens.borrow_mut();
            // 移除已不存在的旧类
            for c in prev.difference(&new_tokens) {
                let _ = list.remove_1(c);
            }
            // 添加新类
            for c in new_tokens.difference(&prev) {
                let _ = list.add_1(c);
            }
            *prev = new_tokens;
        }
    });
}

fn apply_combined_styles_internal(
    el: &Element,
    statics: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    properties: Vec<(Cow<'static, str>, Rx<String>)>,
    sheets: Vec<Rx<String>>,
) {
    let style = match get_style_decl(el) {
        Some(s) => s,
        None => return,
    };

    // 1. 立即应用所有静态样式项
    for (k, v) in &statics {
        let _ = style.set_property(k, v);
    }

    if properties.is_empty() && sheets.is_empty() {
        return;
    }

    // 2. 建立单 Effect 追踪所有响应式样式
    let prev_sheet_keys = Rc::new(RefCell::new(HashSet::<String>::new()));
    let el_clone = el.clone();

    Effect::new(move |_| {
        if let Some(style) = get_style_decl(&el_clone) {
            // 处理单项 Property 绑定
            for (name, rx) in &properties {
                let _ = style.set_property(name, &rx.get());
            }

            // 处理整块响应式样式字符串 (Diff 处理)
            if !sheets.is_empty() {
                let mut new_style_map = std::collections::HashMap::new();
                for rx in &sheets {
                    for (k, v) in parse_style_str(&rx.get()) {
                        new_style_map.insert(k.to_string(), v.to_string());
                    }
                }

                let mut prev = prev_sheet_keys.borrow_mut();
                let new_keys: HashSet<_> = new_style_map.keys().cloned().collect();

                // 移除旧键
                for k in prev.difference(&new_keys) {
                    let _ = style.remove_property(k);
                }
                // 设置新键/更新值
                for (k, v) in new_style_map {
                    let _ = style.set_property(&k, &v);
                }
                *prev = new_keys;
            }
        }
    });
}

// --- Kernel Functions (Non-generic DOM operations) ---

pub(crate) fn set_string_property_internal(el: &Element, name: &str, value: &str, is_prop: bool) {
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
                let _ = el.set_attribute(name, value);
            }
        }
    }
}

pub(crate) fn apply_immediate_bool_internal(el: &Element, name: &str, value: bool, is_prop: bool) {
    if is_prop {
        let _ = js_sys::Reflect::set(el, &JsValue::from_str(name), &JsValue::from_bool(value));
    } else if value {
        let _ = el.set_attribute(name, "");
    } else {
        let _ = el.remove_attribute(name);
    }
}

pub(crate) fn get_style_decl(el: &Element) -> Option<CssStyleDeclaration> {
    if let Some(e) = el.dyn_ref::<HtmlElement>() {
        Some(e.style())
    } else {
        el.dyn_ref::<SvgElement>().map(|e| e.style())
    }
}

pub(crate) fn parse_style_str(s: &str) -> Vec<(Cow<'_, str>, Cow<'_, str>)> {
    s.split(';')
        .filter_map(|rule| {
            let rule = rule.trim();
            if rule.is_empty() {
                None
            } else {
                rule.split_once(':')
                    .map(|(k, v)| (Cow::Borrowed(k.trim()), Cow::Borrowed(v.trim())))
            }
        })
        .collect()
}
