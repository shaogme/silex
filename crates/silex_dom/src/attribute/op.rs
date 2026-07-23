use silex_core::prelude::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Element, HtmlElement, SvgElement};

/// 预定义的 DOM 强类型 Property (Fast-Path)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownProp {
    Value,
    Checked,
    Disabled,
    ReadOnly,
    Required,
}

impl KnownProp {
    pub fn name(self) -> &'static str {
        match self {
            KnownProp::Value => "value",
            KnownProp::Checked => "checked",
            KnownProp::Disabled => "disabled",
            KnownProp::ReadOnly => "readOnly",
            KnownProp::Required => "required",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "value" => Some(KnownProp::Value),
            "checked" => Some(KnownProp::Checked),
            "disabled" => Some(KnownProp::Disabled),
            "readOnly" | "readonly" => Some(KnownProp::ReadOnly),
            "required" => Some(KnownProp::Required),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrTarget {
    /// Standard DOM attributes (setAttribute/removeAttribute)
    Attr,
    /// Direct DOM properties (JS object properties)
    Prop,
    /// Known strong-typed DOM property fast-path
    Known(KnownProp),
}

/// 代表 HTML Attribute 的三种基元状态
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attr {
    /// 移除属性 (remove_attribute)
    Removed,
    /// 布尔标志/空值属性 (set_attribute(name, ""))
    Empty,
    /// 包含字符串值的属性 (set_attribute(name, value))
    String(Cow<'static, str>),
}

impl From<bool> for Attr {
    fn from(b: bool) -> Self {
        if b { Attr::Empty } else { Attr::Removed }
    }
}

impl From<&'static str> for Attr {
    fn from(s: &'static str) -> Self {
        if s.is_empty() {
            Attr::Empty
        } else {
            Attr::String(Cow::Borrowed(s))
        }
    }
}

impl From<String> for Attr {
    fn from(s: String) -> Self {
        if s.is_empty() {
            Attr::Empty
        } else {
            Attr::String(Cow::Owned(s))
        }
    }
}

impl From<Cow<'static, str>> for Attr {
    fn from(s: Cow<'static, str>) -> Self {
        if s.is_empty() {
            Attr::Empty
        } else {
            Attr::String(s)
        }
    }
}

impl<T: Into<Attr>> From<Option<T>> for Attr {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Attr::Removed,
        }
    }
}

#[derive(Clone)]
pub enum AttrData {
    // --- Static Values ---
    StaticAttr(Attr),
    StaticJs(JsValue),

    // --- Reactive Values ---
    ReactiveAttr(Rx<Attr>),
    ReactiveJs(Rx<JsValue>),
}

impl std::fmt::Debug for AttrData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaticAttr(a) => f.debug_tuple("StaticAttr").field(a).finish(),
            Self::StaticJs(js) => f.debug_tuple("StaticJs").field(js).finish(),
            Self::ReactiveAttr(_) => f.write_str("ReactiveAttr(Rx)"),
            Self::ReactiveJs(_) => f.write_str("ReactiveJs(Rx)"),
        }
    }
}

impl PartialEq for AttrData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::StaticAttr(a), Self::StaticAttr(b)) => a == b,
            (Self::StaticJs(a), Self::StaticJs(b)) => a == b,
            (Self::ReactiveAttr(a), Self::ReactiveAttr(b)) => a == b,
            (Self::ReactiveJs(a), Self::ReactiveJs(b)) => a == b,
            _ => false,
        }
    }
}

// --- AttrOp Variant Structs ---

#[derive(Clone, Debug, PartialEq)]
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

impl std::fmt::Debug for ClassToggle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassToggle")
            .field("name", &self.name)
            .field("rx", &"Rx<bool>")
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct StyleProperty {
    pub name: Cow<'static, str>,
    pub rx: Rx<String>,
}

impl std::fmt::Debug for StyleProperty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleProperty")
            .field("name", &self.name)
            .field("rx", &"Rx<String>")
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct CombinedClasses {
    pub statics: Vec<Cow<'static, str>>,
    pub toggles: Vec<(Cow<'static, str>, Rx<bool>)>,
    pub reactives: Vec<Rx<String>>,
}

impl std::fmt::Debug for CombinedClasses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombinedClasses")
            .field("statics", &self.statics)
            .field("toggles_count", &self.toggles.len())
            .field("reactives_count", &self.reactives.len())
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct CombinedStyles {
    pub statics: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    pub properties: Vec<(Cow<'static, str>, Rx<String>)>,
    pub sheets: Vec<Rx<String>>,
}

impl std::fmt::Debug for CombinedStyles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombinedStyles")
            .field("statics", &self.statics)
            .field("properties_count", &self.properties.len())
            .field("sheets_count", &self.sheets.len())
            .finish()
    }
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

impl std::fmt::Debug for AttrOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Update(u) => f.debug_tuple("Update").field(u).finish(),
            Self::SetStaticClasses(c) => f.debug_tuple("SetStaticClasses").field(c).finish(),
            Self::AddClassToggle(ct) => f.debug_tuple("AddClassToggle").field(ct).finish(),
            Self::AddReactiveClasses(_) => f.write_str("AddReactiveClasses(Rx)"),
            Self::SetStaticStyles(s) => f.debug_tuple("SetStaticStyles").field(s).finish(),
            Self::BindStyleProperty(sp) => f.debug_tuple("BindStyleProperty").field(sp).finish(),
            Self::BindReactiveStyleSheet(_) => f.write_str("BindReactiveStyleSheet(Rx)"),
            Self::CombinedClasses(cc) => f.debug_tuple("CombinedClasses").field(cc).finish(),
            Self::CombinedStyles(cs) => f.debug_tuple("CombinedStyles").field(cs).finish(),
            Self::Sequence(seq) => f.debug_tuple("Sequence").field(seq).finish(),
            Self::Custom(_) => f.write_str("Custom(Rc<Fn>)"),
            Self::Noop => f.write_str("Noop"),
        }
    }
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
                let prev_classes = Rc::new(RefCell::new(HashSet::<String>::new()));
                let list = el.class_list();
                Effect::new(move |_| {
                    let value = rx.get();
                    let mut prev = prev_classes.borrow_mut();
                    let new_tokens: HashSet<&str> = value.split_whitespace().collect();

                    prev.retain(|c| {
                        if !new_tokens.contains(c.as_str()) {
                            let _ = list.remove_1(c);
                            false
                        } else {
                            true
                        }
                    });

                    for token in new_tokens {
                        if !prev.contains(token) {
                            let _ = list.add_1(token);
                            prev.insert(token.to_string());
                        }
                    }
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
                        let new_style_map: std::collections::HashMap<&str, &str> =
                            params.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();

                        prev.retain(|k| {
                            if !new_style_map.contains_key(k.as_str()) {
                                let _ = style.remove_property(k);
                                false
                            } else {
                                true
                            }
                        });

                        for (k, v) in new_style_map {
                            let _ = style.set_property(k, v);
                            if !prev.contains(k) {
                                prev.insert(k.to_string());
                            }
                        }
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
    match data {
        AttrData::StaticAttr(attr) => {
            apply_attr_with_target_internal(el, name, target, &attr);
        }
        AttrData::StaticJs(value) => {
            let _ = js_sys::Reflect::set(el, &JsValue::from_str(name), &value);
        }
        AttrData::ReactiveAttr(rx) => {
            let el = el.clone();
            let name = name.to_string();
            Effect::new(move |_| {
                apply_attr_with_target_internal(&el, &name, target, &rx.get());
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
    let prev_toggles = Rc::new(RefCell::new(vec![None::<bool>; toggles.len()]));
    let prev_reactive_tokens = Rc::new(RefCell::new(HashSet::<String>::new()));
    let el_clone = el.clone();

    Effect::new(move |_| {
        let list = el_clone.class_list();

        // 处理所有 Toggle (如 .class_toggle)，仅在状态改变时才更新 DOM
        let mut prev_t = prev_toggles.borrow_mut();
        for (i, (name, rx)) in toggles.iter().enumerate() {
            let val = rx.get();
            if prev_t[i] != Some(val) {
                if val {
                    let _ = list.add_1(name);
                } else {
                    let _ = list.remove_1(name);
                }
                prev_t[i] = Some(val);
            }
        }

        // 处理所有响应式字符串类 (需要 Diff 算法以支持正确删除旧类)
        if !reactives.is_empty() {
            let reactive_strings: Vec<String> = reactives.iter().map(|rx| rx.get()).collect();
            let mut new_tokens = HashSet::new();
            for s in &reactive_strings {
                for token in s.split_whitespace() {
                    new_tokens.insert(token);
                }
            }

            let mut prev = prev_reactive_tokens.borrow_mut();
            prev.retain(|c| {
                if !new_tokens.contains(c.as_str()) {
                    let _ = list.remove_1(c);
                    false
                } else {
                    true
                }
            });

            for token in new_tokens {
                if !prev.contains(token) {
                    let _ = list.add_1(token);
                    prev.insert(token.to_string());
                }
            }
        }
    });
}

fn apply_combined_styles_internal(
    el: &Element,
    statics: Vec<(Cow<'static, str>, Cow<'static, str>)>,
    properties: Vec<(Cow<'static, str>, Rx<String>)>,
    sheets: Vec<Rx<String>>,
) {
    let Some(style) = get_style_decl(el) else {
        return;
    };

    // 1. 立即应用所有静态样式项
    for (k, v) in &statics {
        let _ = style.set_property(k, v);
    }

    if properties.is_empty() && sheets.is_empty() {
        return;
    }

    // 2. 建立单 Effect 追踪所有响应式样式
    let prev_props = Rc::new(RefCell::new(vec![None::<String>; properties.len()]));
    let prev_sheet_keys = Rc::new(RefCell::new(HashSet::<String>::new()));
    let el_clone = el.clone();

    Effect::new(move |_| {
        if let Some(style) = get_style_decl(&el_clone) {
            // 处理单项 Property 绑定 (仅在值发生变化时更新 DOM)
            let mut prev_p = prev_props.borrow_mut();
            for (i, (name, rx)) in properties.iter().enumerate() {
                let val = rx.get();
                if prev_p[i].as_deref() != Some(&val) {
                    let _ = style.set_property(name, &val);
                    prev_p[i] = Some(val);
                }
            }

            // 处理整块响应式样式字符串 (Diff 处理)
            if !sheets.is_empty() {
                let sheet_strings: Vec<String> = sheets.iter().map(|rx| rx.get()).collect();
                let mut new_style_map = std::collections::HashMap::new();
                for s in &sheet_strings {
                    for (k, v) in parse_style_str(s) {
                        new_style_map.insert(k.into_owned(), v.into_owned());
                    }
                }

                let mut prev = prev_sheet_keys.borrow_mut();
                let new_keys: HashSet<&str> = new_style_map.keys().map(|k| k.as_str()).collect();

                prev.retain(|k| {
                    if !new_keys.contains(k.as_str()) {
                        let _ = style.remove_property(k);
                        false
                    } else {
                        true
                    }
                });

                for (k, v) in new_style_map {
                    let _ = style.set_property(&k, &v);
                    if !prev.contains(&k) {
                        prev.insert(k);
                    }
                }
            }
        }
    });
}

// --- Kernel Functions (Non-generic DOM operations) ---

pub(crate) fn apply_attr_internal(el: &Element, name: &str, attr: &Attr) {
    if name.is_empty() {
        return;
    }
    match attr {
        Attr::Removed => {
            let _ = el.remove_attribute(name);
        }
        Attr::Empty => {
            let _ = el.set_attribute(name, "");
        }
        Attr::String(val) => match name {
            "class" => el.set_class_name(val),
            "style" => {
                if let Some(style) = get_style_decl(el) {
                    style.set_css_text(val);
                }
            }
            _ => {
                let _ = el.set_attribute(name, val);
            }
        },
    }
}

pub(crate) fn apply_attr_with_target_internal(
    el: &Element,
    name: &str,
    target: AttrTarget,
    attr: &Attr,
) {
    let known_prop = match target {
        AttrTarget::Known(kp) => Some(kp),
        AttrTarget::Prop => KnownProp::parse(name),
        AttrTarget::Attr => None,
    };

    if let Some(prop) = known_prop {
        if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
            match prop {
                KnownProp::Value => {
                    let val = match attr {
                        Attr::Removed | Attr::Empty => "",
                        Attr::String(s) => s.as_ref(),
                    };
                    input.set_value(val);
                    return;
                }
                KnownProp::Checked => {
                    let is_checked = matches!(attr, Attr::Empty | Attr::String(_));
                    input.set_checked(is_checked);
                    return;
                }
                KnownProp::Disabled => {
                    let is_disabled = matches!(attr, Attr::Empty | Attr::String(_));
                    input.set_disabled(is_disabled);
                    if is_disabled {
                        let _ = el.set_attribute("disabled", "");
                    } else {
                        let _ = el.remove_attribute("disabled");
                    }
                    return;
                }
                KnownProp::ReadOnly => {
                    let is_readonly = matches!(attr, Attr::Empty | Attr::String(_));
                    input.set_read_only(is_readonly);
                    if is_readonly {
                        let _ = el.set_attribute("readonly", "");
                    } else {
                        let _ = el.remove_attribute("readonly");
                    }
                    return;
                }
                KnownProp::Required => {
                    let is_required = matches!(attr, Attr::Empty | Attr::String(_));
                    input.set_required(is_required);
                    if is_required {
                        let _ = el.set_attribute("required", "");
                    } else {
                        let _ = el.remove_attribute("required");
                    }
                    return;
                }
            }
        } else if let Some(textarea) = el.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            match prop {
                KnownProp::Value => {
                    let val = match attr {
                        Attr::Removed | Attr::Empty => "",
                        Attr::String(s) => s.as_ref(),
                    };
                    textarea.set_value(val);
                    return;
                }
                KnownProp::Disabled => {
                    let is_disabled = matches!(attr, Attr::Empty | Attr::String(_));
                    textarea.set_disabled(is_disabled);
                    if is_disabled {
                        let _ = el.set_attribute("disabled", "");
                    } else {
                        let _ = el.remove_attribute("disabled");
                    }
                    return;
                }
                KnownProp::ReadOnly => {
                    let is_readonly = matches!(attr, Attr::Empty | Attr::String(_));
                    textarea.set_read_only(is_readonly);
                    if is_readonly {
                        let _ = el.set_attribute("readonly", "");
                    } else {
                        let _ = el.remove_attribute("readonly");
                    }
                    return;
                }
                KnownProp::Required => {
                    let is_required = matches!(attr, Attr::Empty | Attr::String(_));
                    textarea.set_required(is_required);
                    if is_required {
                        let _ = el.set_attribute("required", "");
                    } else {
                        let _ = el.remove_attribute("required");
                    }
                    return;
                }
                _ => {}
            }
        } else if let Some(select) = el.dyn_ref::<web_sys::HtmlSelectElement>() {
            match prop {
                KnownProp::Value => {
                    let val = match attr {
                        Attr::Removed | Attr::Empty => "",
                        Attr::String(s) => s.as_ref(),
                    };
                    select.set_value(val);
                    return;
                }
                KnownProp::Disabled => {
                    let is_disabled = matches!(attr, Attr::Empty | Attr::String(_));
                    select.set_disabled(is_disabled);
                    if is_disabled {
                        let _ = el.set_attribute("disabled", "");
                    } else {
                        let _ = el.remove_attribute("disabled");
                    }
                    return;
                }
                KnownProp::Required => {
                    let is_required = matches!(attr, Attr::Empty | Attr::String(_));
                    select.set_required(is_required);
                    if is_required {
                        let _ = el.set_attribute("required", "");
                    } else {
                        let _ = el.remove_attribute("required");
                    }
                    return;
                }
                _ => {}
            }
        } else if let Some(button) = el.dyn_ref::<web_sys::HtmlButtonElement>() {
            if prop == KnownProp::Disabled {
                let is_disabled = matches!(attr, Attr::Empty | Attr::String(_));
                button.set_disabled(is_disabled);
                if is_disabled {
                    let _ = el.set_attribute("disabled", "");
                } else {
                    let _ = el.remove_attribute("disabled");
                }
                return;
            }
        } else if let Some(option) = el.dyn_ref::<web_sys::HtmlOptionElement>() {
            match prop {
                KnownProp::Value => {
                    let val = match attr {
                        Attr::Removed | Attr::Empty => "",
                        Attr::String(s) => s.as_ref(),
                    };
                    option.set_value(val);
                    return;
                }
                KnownProp::Checked => {
                    let is_checked = matches!(attr, Attr::Empty | Attr::String(_));
                    option.set_selected(is_checked);
                    return;
                }
                KnownProp::Disabled => {
                    let is_disabled = matches!(attr, Attr::Empty | Attr::String(_));
                    option.set_disabled(is_disabled);
                    if is_disabled {
                        let _ = el.set_attribute("disabled", "");
                    } else {
                        let _ = el.remove_attribute("disabled");
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    apply_attr_internal(el, name, attr);
}

pub(crate) fn set_string_property_internal(el: &Element, name: &str, value: &str, is_prop: bool) {
    let target = if is_prop {
        AttrTarget::Prop
    } else {
        AttrTarget::Attr
    };
    apply_attr_with_target_internal(el, name, target, &Attr::from(value.to_string()));
}

pub(crate) fn apply_immediate_bool_internal(el: &Element, name: &str, value: bool, is_prop: bool) {
    let target = if is_prop {
        AttrTarget::Prop
    } else {
        AttrTarget::Attr
    };
    apply_attr_with_target_internal(el, name, target, &Attr::from(value));
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
