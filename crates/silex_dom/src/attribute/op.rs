use silex_core::RuntimeInputs;
use silex_core::prelude::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Element, HtmlElement, SvgElement};

use crate::attribute::apply::ApplyTarget;
use crate::view::ViewOwnerToken;

type CustomAttribute<'scope> = Rc<dyn Fn(&Element, &ViewOwnerToken<'scope>) + 'scope>;

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

/// 代表 HTML Attribute 的三种基元状态
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attr<'scope> {
    /// 移除属性 (remove_attribute)
    Removed,
    /// 布尔标志/空值属性 (set_attribute(name, ""))
    Empty,
    /// 包含字符串值的属性 (set_attribute(name, value))
    String(Cow<'scope, str>),
}

impl From<bool> for Attr<'_> {
    fn from(b: bool) -> Self {
        if b { Attr::Empty } else { Attr::Removed }
    }
}

impl<'a> From<&'a str> for Attr<'a> {
    fn from(s: &'a str) -> Self {
        if s.is_empty() {
            Attr::Empty
        } else {
            Attr::String(Cow::Borrowed(s))
        }
    }
}

impl From<String> for Attr<'_> {
    fn from(s: String) -> Self {
        if s.is_empty() {
            Attr::Empty
        } else {
            Attr::String(Cow::Owned(s))
        }
    }
}

impl<'scope> From<Cow<'scope, str>> for Attr<'scope> {
    fn from(s: Cow<'scope, str>) -> Self {
        if s.is_empty() {
            Attr::Empty
        } else {
            Attr::String(s)
        }
    }
}

impl<'scope, T: Into<Attr<'scope>>> From<Option<T>> for Attr<'scope> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => v.into(),
            None => Attr::Removed,
        }
    }
}

#[derive(Clone)]
pub enum AttrData<'scope> {
    // --- Static Values ---
    StaticAttr(Attr<'scope>),
    StaticJs(JsValue),

    // --- Reactive Values ---
    ReactiveAttr(Rx<'scope, Attr<'scope>>),
    ReactiveString(Rx<'scope, String>),
    ReactiveBool(Rx<'scope, bool>),
    ReactiveOptionString(Rx<'scope, Option<String>>),
    ReactiveJs(Rx<'scope, JsValue>),
}

impl std::fmt::Debug for AttrData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaticAttr(a) => f.debug_tuple("StaticAttr").field(a).finish(),
            Self::StaticJs(js) => f.debug_tuple("StaticJs").field(js).finish(),
            Self::ReactiveAttr(_) => f.write_str("ReactiveAttr(Rx)"),
            Self::ReactiveString(_) => f.write_str("ReactiveString(Rx)"),
            Self::ReactiveBool(_) => f.write_str("ReactiveBool(Rx)"),
            Self::ReactiveOptionString(_) => f.write_str("ReactiveOptionString(Rx)"),
            Self::ReactiveJs(_) => f.write_str("ReactiveJs(Rx)"),
        }
    }
}

impl PartialEq for AttrData<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::StaticAttr(a), Self::StaticAttr(b)) => a == b,
            (Self::StaticJs(a), Self::StaticJs(b)) => a == b,
            (Self::ReactiveAttr(_), Self::ReactiveAttr(_)) => false,
            (Self::ReactiveString(_), Self::ReactiveString(_)) => false,
            (Self::ReactiveBool(_), Self::ReactiveBool(_)) => false,
            (Self::ReactiveOptionString(_), Self::ReactiveOptionString(_)) => false,
            (Self::ReactiveJs(_), Self::ReactiveJs(_)) => false,
            _ => false,
        }
    }
}

// --- AttrOp Variant Structs ---

#[derive(Clone, Debug, PartialEq)]
pub struct AttrUpdate<'scope> {
    pub target: ApplyTarget,
    pub data: AttrData<'scope>,
}

#[derive(Clone)]
pub struct ClassToggle<'scope> {
    pub name: Cow<'scope, str>,
    pub rx: Rx<'scope, bool>,
}

impl PartialEq for ClassToggle<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.rx == other.rx
    }
}

impl std::fmt::Debug for ClassToggle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassToggle")
            .field("name", &self.name)
            .field("rx", &"Rx<bool>")
            .finish()
    }
}

#[derive(Clone)]
pub struct StyleProperty<'scope> {
    pub name: Cow<'scope, str>,
    pub rx: Rx<'scope, String>,
}

impl PartialEq for StyleProperty<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.rx == other.rx
    }
}

impl std::fmt::Debug for StyleProperty<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleProperty")
            .field("name", &self.name)
            .field("rx", &"Rx<String>")
            .finish()
    }
}

#[derive(Clone)]
pub struct CombinedClasses<'scope> {
    pub statics: Vec<Cow<'scope, str>>,
    pub toggles: Vec<(Cow<'scope, str>, Rx<'scope, bool>)>,
    pub reactives: Vec<Rx<'scope, String>>,
}

impl PartialEq for CombinedClasses<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.reactives == other.reactives
            && self.toggles == other.toggles
    }
}

impl std::fmt::Debug for CombinedClasses<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombinedClasses")
            .field("statics", &self.statics)
            .field("toggles_count", &self.toggles.len())
            .field("reactives_count", &self.reactives.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct CombinedStyles<'scope> {
    pub statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
    pub properties: Vec<(Cow<'scope, str>, Rx<'scope, String>)>,
    pub sheets: Vec<Rx<'scope, String>>,
}

impl PartialEq for CombinedStyles<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.sheets == other.sheets
            && self.properties == other.properties
    }
}

impl std::fmt::Debug for CombinedStyles<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombinedStyles")
            .field("statics", &self.statics)
            .field("properties_count", &self.properties.len())
            .field("sheets_count", &self.sheets.len())
            .finish()
    }
}

#[derive(Clone)]
pub enum AttrOp<'scope> {
    /// Unified update for attributes and properties (Static or Reactive)
    Update(AttrUpdate<'scope>),

    /// Consolidated class operations (statics, toggles, reactives)
    CombinedClasses(CombinedClasses<'scope>),

    /// Consolidated style operations (statics, properties, sheets)
    CombinedStyles(CombinedStyles<'scope>),

    /// Sequence of operations
    Sequence(Vec<AttrOp<'scope>>),

    /// Custom closure execution
    Custom(CustomAttribute<'scope>),

    /// Custom closure with inputs declared outside the closure body.
    CustomWithInputs {
        inputs: RuntimeInputs,
        callback: CustomAttribute<'scope>,
    },

    /// No operation
    Noop,
}

impl std::fmt::Debug for AttrOp<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Update(u) => f.debug_tuple("Update").field(u).finish(),
            Self::CombinedClasses(cc) => f.debug_tuple("CombinedClasses").field(cc).finish(),
            Self::CombinedStyles(cs) => f.debug_tuple("CombinedStyles").field(cs).finish(),
            Self::Sequence(seq) => f.debug_tuple("Sequence").field(seq).finish(),
            Self::Custom(_) => f.write_str("Custom(Rc<Fn>)"),
            Self::CustomWithInputs { inputs, .. } => f
                .debug_struct("CustomWithInputs")
                .field("inputs", inputs)
                .field("callback", &"Rc<Fn>")
                .finish(),
            Self::Noop => f.write_str("Noop"),
        }
    }
}

impl PartialEq for AttrOp<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Update(a), Self::Update(b)) => a == b,
            (Self::CombinedClasses(a), Self::CombinedClasses(b)) => a == b,
            (Self::CombinedStyles(a), Self::CombinedStyles(b)) => a == b,
            (Self::Sequence(a), Self::Sequence(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => Rc::ptr_eq(a, b),
            (
                Self::CustomWithInputs {
                    inputs: inputs_a,
                    callback: callback_a,
                },
                Self::CustomWithInputs {
                    inputs: inputs_b,
                    callback: callback_b,
                },
            ) => inputs_a == inputs_b && Rc::ptr_eq(callback_a, callback_b),
            (Self::Noop, Self::Noop) => true,
            _ => false,
        }
    }
}

impl<'scope> AttrOp<'scope> {
    pub fn static_class(c: Cow<'scope, str>) -> Self {
        AttrOp::CombinedClasses(CombinedClasses {
            statics: vec![c],
            toggles: Vec::new(),
            reactives: Vec::new(),
        })
    }

    pub fn static_classes(c: Vec<Cow<'scope, str>>) -> Self {
        AttrOp::CombinedClasses(CombinedClasses {
            statics: c,
            toggles: Vec::new(),
            reactives: Vec::new(),
        })
    }

    pub fn class_toggle(name: Cow<'scope, str>, rx: Rx<'scope, bool>) -> Self {
        AttrOp::CombinedClasses(CombinedClasses {
            statics: Vec::new(),
            toggles: vec![(name, rx)],
            reactives: Vec::new(),
        })
    }

    pub fn reactive_classes(rx: Rx<'scope, String>) -> Self {
        AttrOp::CombinedClasses(CombinedClasses {
            statics: Vec::new(),
            toggles: Vec::new(),
            reactives: vec![rx],
        })
    }

    pub fn static_styles(styles: Vec<(Cow<'scope, str>, Cow<'scope, str>)>) -> Self {
        AttrOp::CombinedStyles(CombinedStyles {
            statics: styles,
            properties: Vec::new(),
            sheets: Vec::new(),
        })
    }

    pub fn style_property(name: Cow<'scope, str>, rx: Rx<'scope, String>) -> Self {
        AttrOp::CombinedStyles(CombinedStyles {
            statics: Vec::new(),
            properties: vec![(name, rx)],
            sheets: Vec::new(),
        })
    }

    pub fn reactive_stylesheet(rx: Rx<'scope, String>) -> Self {
        AttrOp::CombinedStyles(CombinedStyles {
            statics: Vec::new(),
            properties: Vec::new(),
            sheets: vec![rx],
        })
    }

    pub fn custom_with_inputs(
        inputs: RuntimeInputs,
        callback: impl Fn(&Element, &ViewOwnerToken<'scope>) + 'scope,
    ) -> Self {
        Self::CustomWithInputs {
            inputs,
            callback: Rc::new(callback),
        }
    }

    pub(crate) fn runtime_inputs(&self) -> RuntimeInputs {
        let mut inputs = RuntimeInputs::new();
        match self {
            AttrOp::Update(AttrUpdate { data, .. }) => match data {
                AttrData::ReactiveAttr(rx) => inputs.extend(&rx.runtime_inputs()),
                AttrData::ReactiveString(rx) => inputs.extend(&rx.runtime_inputs()),
                AttrData::ReactiveBool(rx) => inputs.extend(&rx.runtime_inputs()),
                AttrData::ReactiveOptionString(rx) => inputs.extend(&rx.runtime_inputs()),
                AttrData::ReactiveJs(rx) => inputs.extend(&rx.runtime_inputs()),
                AttrData::StaticAttr(_) | AttrData::StaticJs(_) => {}
            },
            AttrOp::CombinedClasses(CombinedClasses {
                toggles, reactives, ..
            }) => {
                for (_, rx) in toggles {
                    inputs.extend(&rx.runtime_inputs());
                }
                for rx in reactives {
                    inputs.extend(&rx.runtime_inputs());
                }
            }
            AttrOp::CombinedStyles(CombinedStyles {
                properties, sheets, ..
            }) => {
                for (_, rx) in properties {
                    inputs.extend(&rx.runtime_inputs());
                }
                for rx in sheets {
                    inputs.extend(&rx.runtime_inputs());
                }
            }
            AttrOp::Sequence(ops) => {
                for op in ops {
                    inputs.extend(&op.runtime_inputs());
                }
            }
            AttrOp::Custom(_) | AttrOp::Noop => {}
            AttrOp::CustomWithInputs {
                inputs: declared, ..
            } => {
                inputs.extend(declared);
            }
        }
        inputs
    }

    pub fn apply(self, el: &Element, owner: &ViewOwnerToken<'scope>) {
        let inputs = self.runtime_inputs();
        if let Err(error) = owner.validate_inputs(&inputs) {
            owner.report_error(error);
            return;
        }
        self.apply_unchecked(el, owner);
    }

    fn apply_unchecked(self, el: &Element, owner: &ViewOwnerToken<'scope>) {
        match self {
            AttrOp::Update(AttrUpdate { target, data }) => {
                apply_update_internal(el, target, data, owner);
            }
            AttrOp::CombinedClasses(CombinedClasses {
                statics,
                toggles,
                reactives,
            }) => {
                apply_combined_classes_internal(el, statics, toggles, reactives, owner);
            }
            AttrOp::CombinedStyles(CombinedStyles {
                statics,
                properties,
                sheets,
            }) => {
                apply_combined_styles_internal(el, statics, properties, sheets, owner);
            }
            AttrOp::Sequence(ops) => {
                for op in ops {
                    op.apply_unchecked(el, owner);
                }
            }
            AttrOp::Custom(f) => {
                f(el, owner);
            }
            AttrOp::CustomWithInputs { callback, .. } => {
                callback(el, owner);
            }
            AttrOp::Noop => {}
        }
    }
}

fn apply_update_internal<'scope>(
    el: &Element,
    target: ApplyTarget,
    data: AttrData<'scope>,
    owner: &ViewOwnerToken<'scope>,
) {
    let name = target.attr_name().to_string();
    match data {
        AttrData::StaticAttr(attr) => {
            apply_attr_with_target_internal(el, &name, target, &attr);
        }
        AttrData::StaticJs(value) => {
            let _ = js_sys::Reflect::set(el, &JsValue::from_str(&name), &value);
        }
        AttrData::ReactiveAttr(rx) => {
            let el = el.clone();
            if let Err(error) = owner.effect_from(
                rx.runtime_inputs(),
                Box::new(move || {
                    let name = target.attr_name();
                    apply_attr_with_target_internal(&el, name, target.clone(), &rx.get());
                }),
            ) {
                owner.report_error(error);
            }
        }
        AttrData::ReactiveString(rx) => {
            let el = el.clone();
            if let Err(error) = owner.effect_from(
                rx.runtime_inputs(),
                Box::new(move || {
                    let name = target.attr_name();
                    let val = rx.get();
                    apply_attr_with_target_internal(&el, name, target.clone(), &Attr::from(val));
                }),
            ) {
                owner.report_error(error);
            }
        }
        AttrData::ReactiveBool(rx) => {
            let el = el.clone();
            if let Err(error) = owner.effect_from(
                rx.runtime_inputs(),
                Box::new(move || {
                    let name = target.attr_name();
                    let val = rx.get();
                    apply_attr_with_target_internal(&el, name, target.clone(), &Attr::from(val));
                }),
            ) {
                owner.report_error(error);
            }
        }
        AttrData::ReactiveOptionString(rx) => {
            let el = el.clone();
            if let Err(error) = owner.effect_from(
                rx.runtime_inputs(),
                Box::new(move || {
                    let name = target.attr_name();
                    let val = rx.get();
                    let attr = match val {
                        Some(s) => Attr::from(s),
                        None => Attr::Removed,
                    };
                    apply_attr_with_target_internal(&el, name, target.clone(), &attr);
                }),
            ) {
                owner.report_error(error);
            }
        }
        AttrData::ReactiveJs(rx) => {
            let el = el.clone();
            if let Err(error) = owner.effect_from(
                rx.runtime_inputs(),
                Box::new(move || {
                    let _ = js_sys::Reflect::set(&el, &JsValue::from_str(&name), &rx.get());
                }),
            ) {
                owner.report_error(error);
            }
        }
    }
}

// --- Kernel Implementation Functions for Combined Op ---

fn apply_combined_classes_internal<'scope>(
    el: &Element,
    statics: Vec<Cow<'scope, str>>,
    toggles: Vec<(Cow<'scope, str>, Rx<'scope, bool>)>,
    reactives: Vec<Rx<'scope, String>>,
    owner: &ViewOwnerToken<'scope>,
) {
    let list = el.class_list();
    // 1. 立即应用所有静态类（非响应式，仅执行一次）
    for s in &statics {
        let _ = list.add_1(s);
    }

    if toggles.is_empty() && reactives.is_empty() {
        return;
    }

    let static_tokens: HashSet<String> = statics
        .iter()
        .flat_map(|class| class.split_whitespace().map(str::to_owned))
        .collect();
    let mut inputs = RuntimeInputs::new();
    for (_, rx) in &toggles {
        inputs.extend(&rx.runtime_inputs());
    }
    for rx in &reactives {
        inputs.extend(&rx.runtime_inputs());
    }

    // 2. 建立单 Effect 追踪所有响应式部分
    let prev_dynamic_tokens = Rc::new(RefCell::new(HashSet::<String>::new()));
    let prev_dynamic_tokens_for_cleanup = prev_dynamic_tokens.clone();
    let static_tokens_for_update = static_tokens.clone();
    let static_tokens_for_cleanup = static_tokens.clone();
    let el_clone = el.clone();
    let el_for_cleanup = el.clone();

    if let Err(error) = owner.effect_from(
        inputs,
        Box::new(move || {
            let list = el_clone.class_list();

            // 先合并所有动态来源。一个 token 可能同时来自 toggle 与 reactive
            // class，只有它不再被任何动态来源提供时才能从 DOM 中移除。
            let mut new_dynamic_tokens = HashSet::new();
            for (name, rx) in &toggles {
                if rx.get() {
                    new_dynamic_tokens.insert(name.to_string());
                }
            }

            for rx in &reactives {
                for token in rx.get().split_whitespace() {
                    new_dynamic_tokens.insert(token.to_string());
                }
            }

            let mut prev = prev_dynamic_tokens.borrow_mut();
            let previous = prev.clone();

            // 先添加新增加的 Class，确保样式/过渡声明（transition）无缝连接，
            // 不因无类中间态产生闪烁或动画打断。
            for token in new_dynamic_tokens.difference(&previous) {
                let _ = list.add_1(token);
            }

            // 只删除已经不再由任何动态来源提供的旧 Class；静态 Class 即使
            // 同名，也必须继续保留。
            for token in previous.difference(&new_dynamic_tokens) {
                if !static_tokens_for_update.contains(token) {
                    let _ = list.remove_1(token);
                }
            }

            *prev = new_dynamic_tokens;
        }),
    ) {
        owner.report_error(error);
    }

    if let Err(error) = owner.on_cleanup(Box::new(move || {
        let dynamic_tokens = prev_dynamic_tokens_for_cleanup.borrow().clone();
        let list = el_for_cleanup.class_list();
        for token in dynamic_tokens {
            if !static_tokens_for_cleanup.contains(&token) {
                let _ = list.remove_1(&token);
            }
        }
    })) {
        owner.report_error(error);
    }
}

fn apply_combined_styles_internal<'scope>(
    el: &Element,
    statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
    properties: Vec<(Cow<'scope, str>, Rx<'scope, String>)>,
    sheets: Vec<Rx<'scope, String>>,
    owner: &ViewOwnerToken<'scope>,
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

    let mut inputs = RuntimeInputs::new();
    for (_, rx) in &properties {
        inputs.extend(&rx.runtime_inputs());
    }
    for rx in &sheets {
        inputs.extend(&rx.runtime_inputs());
    }

    // 2. 建立单 Effect 追踪所有响应式样式
    let prev_props = Rc::new(RefCell::new(vec![None::<String>; properties.len()]));
    let prev_sheet_keys = Rc::new(RefCell::new(HashSet::<String>::new()));
    let el_clone = el.clone();
    let property_names: Vec<String> = properties
        .iter()
        .map(|(name, _)| name.to_string())
        .collect();
    let prev_sheet_keys_for_cleanup = prev_sheet_keys.clone();
    let el_for_cleanup = el.clone();

    if let Err(error) = owner.effect_from(
        inputs,
        Box::new(move || {
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
                    let new_keys: HashSet<&str> =
                        new_style_map.keys().map(|k| k.as_str()).collect();

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
        }),
    ) {
        owner.report_error(error);
    }

    if let Err(error) = owner.on_cleanup(Box::new(move || {
        if let Some(style) = get_style_decl(&el_for_cleanup) {
            for name in property_names {
                let _ = style.remove_property(&name);
            }
            for name in prev_sheet_keys_for_cleanup.borrow().iter() {
                let _ = style.remove_property(name);
            }
        }
    })) {
        owner.report_error(error);
    }
}

// --- Kernel Functions (Non-generic DOM operations) ---

pub(crate) fn apply_attr_internal(el: &Element, name: &str, attr: &Attr<'_>) {
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
    target: ApplyTarget,
    attr: &Attr<'_>,
) {
    let known_prop = match target {
        ApplyTarget::Known(kp) => Some(kp),
        _ => None,
    };

    if let Some(prop) = known_prop {
        let is_truthy = matches!(attr, Attr::Empty | Attr::String(_));
        let attr_str = match attr {
            Attr::Removed | Attr::Empty => "",
            Attr::String(s) => s.as_ref(),
        };

        macro_rules! set_bool_and_sync {
            ($attr_name:expr, $expr:expr) => {{
                $expr;
                if is_truthy {
                    let _ = el.set_attribute($attr_name, "");
                } else {
                    let _ = el.remove_attribute($attr_name);
                }
            }};
        }

        let handled = if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
            match prop {
                KnownProp::Value => {
                    input.set_value(attr_str);
                    true
                }
                KnownProp::Checked => {
                    input.set_checked(is_truthy);
                    true
                }
                KnownProp::Disabled => {
                    set_bool_and_sync!("disabled", input.set_disabled(is_truthy));
                    true
                }
                KnownProp::ReadOnly => {
                    set_bool_and_sync!("readonly", input.set_read_only(is_truthy));
                    true
                }
                KnownProp::Required => {
                    set_bool_and_sync!("required", input.set_required(is_truthy));
                    true
                }
            }
        } else if let Some(textarea) = el.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            match prop {
                KnownProp::Value => {
                    textarea.set_value(attr_str);
                    true
                }
                KnownProp::Disabled => {
                    set_bool_and_sync!("disabled", textarea.set_disabled(is_truthy));
                    true
                }
                KnownProp::ReadOnly => {
                    set_bool_and_sync!("readonly", textarea.set_read_only(is_truthy));
                    true
                }
                KnownProp::Required => {
                    set_bool_and_sync!("required", textarea.set_required(is_truthy));
                    true
                }
                _ => false,
            }
        } else if let Some(select) = el.dyn_ref::<web_sys::HtmlSelectElement>() {
            match prop {
                KnownProp::Value => {
                    select.set_value(attr_str);
                    true
                }
                KnownProp::Disabled => {
                    set_bool_and_sync!("disabled", select.set_disabled(is_truthy));
                    true
                }
                KnownProp::Required => {
                    set_bool_and_sync!("required", select.set_required(is_truthy));
                    true
                }
                _ => false,
            }
        } else if let Some(button) = el.dyn_ref::<web_sys::HtmlButtonElement>() {
            if prop == KnownProp::Disabled {
                set_bool_and_sync!("disabled", button.set_disabled(is_truthy));
                true
            } else {
                false
            }
        } else if let Some(option) = el.dyn_ref::<web_sys::HtmlOptionElement>() {
            match prop {
                KnownProp::Value => {
                    option.set_value(attr_str);
                    true
                }
                KnownProp::Checked => {
                    option.set_selected(is_truthy);
                    true
                }
                KnownProp::Disabled => {
                    set_bool_and_sync!("disabled", option.set_disabled(is_truthy));
                    true
                }
                _ => false,
            }
        } else {
            false
        };

        if handled {
            return;
        }
    }

    apply_attr_internal(el, name, attr);
}

pub(crate) fn set_string_property_internal(el: &Element, name: &str, value: &str, is_prop: bool) {
    let target = if is_prop {
        ApplyTarget::prop(name.to_string())
    } else {
        ApplyTarget::attr(name.to_string())
    };
    apply_attr_with_target_internal(el, name, target, &Attr::from(value.to_string()));
}

pub(crate) fn apply_immediate_bool_internal(el: &Element, name: &str, value: bool, is_prop: bool) {
    let target = if is_prop {
        ApplyTarget::prop(name.to_string())
    } else {
        ApplyTarget::attr(name.to_string())
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
