use silex_core::prelude::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Element, HtmlElement, SvgElement};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrTarget {
    /// Standard DOM attributes (setAttribute/removeAttribute)
    Attr,
    /// Direct DOM properties (JS object properties)
    Prop,
}

#[derive(Clone)]
pub enum AttrData {
    // --- Static Values ---
    StaticString(Cow<'static, str>),
    StaticBool(bool),
    StaticJs(JsValue),

    // --- Reactive Values ---
    ReactiveString(Rx<String>),
    ReactiveBool(Rx<bool>),
    ReactiveJs(Rx<JsValue>),
}

impl PartialEq for AttrData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::StaticString(a), Self::StaticString(b)) => a == b,
            (Self::StaticBool(a), Self::StaticBool(b)) => a == b,
            (Self::StaticJs(a), Self::StaticJs(b)) => a == b,
            (Self::ReactiveString(a), Self::ReactiveString(b)) => a == b,
            (Self::ReactiveBool(a), Self::ReactiveBool(b)) => a == b,
            (Self::ReactiveJs(a), Self::ReactiveJs(b)) => a == b,
            _ => false,
        }
    }
}

// --- AttrOp Variant Structs ---

#[derive(Clone, PartialEq)]
pub struct AttrUpdate {
    pub name: Cow<'static, str>,
    pub target: AttrTarget,
    pub data: AttrData,
}

#[derive(Clone, PartialEq)]
pub struct ClassToggle {
    pub name: Cow<'static, str>,
    pub rx: Rx<bool>,
}

#[derive(Clone, PartialEq)]
pub struct StyleProperty {
    pub name: Cow<'static, str>,
    pub rx: Rx<String>,
}

#[derive(Clone, PartialEq)]
pub struct CombinedClasses {
    pub statics: Vec<Cow<'static, str>>,
    pub toggles: Vec<(Cow<'static, str>, Rx<bool>)>,
    pub reactives: Vec<Rx<String>>,
}

#[derive(Clone, PartialEq)]
pub struct CombinedStyles {
    pub statics: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    pub properties: Vec<(Cow<'static, str>, Rx<String>)>,
    pub sheets: Vec<Rx<String>>,
}

#[derive(Clone)]
pub enum AttrOp {
    /// Unified update for attributes and properties (Static or Reactive)
    Update(AttrUpdate),

    // --- Class 专项优化（收敛意图） ---
    SetStaticClasses(Vec<Cow<'static, str>>),
    AddClassToggle(ClassToggle),
    AddReactiveClasses(Rx<String>),

    // --- Style 专项优化（收敛意图） ---
    SetStaticStyles(Vec<(Cow<'static, str>, Cow<'static, str>)>),
    BindStyleProperty(StyleProperty),
    BindReactiveStyleSheet(Rx<String>),

    // --- 阶段三：单 Effect 策略优化 (全面转向 AttrOp 的核心) ---
    CombinedClasses(CombinedClasses),
    CombinedStyles(CombinedStyles),

    // --- 集合处理优化（替代部分 Custom 闭包） ---
    Sequence(Vec<AttrOp>),

    // --- 逃逸舱与特殊指令 ---
    Custom(Rc<dyn Fn(&Element)>),
    Noop,
}

impl PartialEq for AttrOp {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Update(a), Self::Update(b)) => a == b,
            (Self::SetStaticClasses(a), Self::SetStaticClasses(b)) => a == b,
            (Self::AddClassToggle(a), Self::AddClassToggle(b)) => a == b,
            (Self::AddReactiveClasses(a), Self::AddReactiveClasses(b)) => a == b,
            (Self::SetStaticStyles(a), Self::SetStaticStyles(b)) => a == b,
            (Self::BindStyleProperty(a), Self::BindStyleProperty(b)) => a == b,
            (Self::BindReactiveStyleSheet(a), Self::BindReactiveStyleSheet(b)) => a == b,
            (Self::CombinedClasses(a), Self::CombinedClasses(b)) => a == b,
            (Self::CombinedStyles(a), Self::CombinedStyles(b)) => a == b,
            (Self::Sequence(a), Self::Sequence(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => Rc::ptr_eq(a, b),
            (Self::Noop, Self::Noop) => true,
            _ => false,
        }
    }
}

impl AttrOp {
    pub fn apply(self, el: &Element) {
        match self {
            AttrOp::Update(AttrUpdate { name, target, data }) => {
                apply_update_internal(el, &name, target, data);
            }
            AttrOp::SetStaticClasses(classes) => {
                let list = el.class_list();
                for c in classes {
                    let _ = list.add_1(&c);
                }
            }
            AttrOp::AddClassToggle(ClassToggle { name, rx }) => {
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
            AttrOp::BindStyleProperty(StyleProperty { name, rx }) => {
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
            AttrOp::Sequence(ops) => {
                for op in ops {
                    op.apply(el);
                }
            }

            AttrOp::Custom(f) => {
                f(el);
            }
            AttrOp::Noop => {}

            // --- 阶段三：合并应用的深度优化 (分发到 Kernel 函数) ---
            AttrOp::CombinedClasses(CombinedClasses {
                statics,
                toggles,
                reactives,
            }) => {
                apply_combined_classes_internal(el, statics, toggles, reactives);
            }
            AttrOp::CombinedStyles(CombinedStyles {
                statics,
                properties,
                sheets,
            }) => {
                apply_combined_styles_internal(el, statics, properties, sheets);
            }
        }
    }
}

fn apply_update_internal(el: &Element, name: &str, target: AttrTarget, data: AttrData) {
    let is_prop = matches!(target, AttrTarget::Prop);
    match data {
        AttrData::StaticString(value) => {
            set_string_property_internal(el, name, &value, is_prop);
        }
        AttrData::StaticBool(value) => {
            apply_immediate_bool_internal(el, name, value, is_prop);
        }
        AttrData::StaticJs(value) => {
            let _ = js_sys::Reflect::set(el, &JsValue::from_str(name), &value);
        }
        AttrData::ReactiveString(rx) => {
            let el = el.clone();
            let name = name.to_string();
            Effect::new(move |_| {
                set_string_property_internal(&el, &name, &rx.get(), is_prop);
            });
        }
        AttrData::ReactiveBool(rx) => {
            let el = el.clone();
            let name = name.to_string();
            Effect::new(move |_| {
                apply_immediate_bool_internal(&el, &name, rx.get(), is_prop);
            });
        }
        AttrData::ReactiveJs(rx) => {
            let el = el.clone();
            let name = name.to_string();
            Effect::new(move |_| {
                let _ = js_sys::Reflect::set(&el, &JsValue::from_str(&name), &rx.get());
            });
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
