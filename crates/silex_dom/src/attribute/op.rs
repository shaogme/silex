use silex_core::prelude::*;
use std::borrow::Cow;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleDeclaration, Element, HtmlElement, SvgElement};

use crate::attribute::apply::{ApplyTarget, ReactiveBindingPlan, ReactiveBindingTarget};
use crate::view::{MountContext, MountErrorHandler, MountOwnerToken};

type CustomAttribute<'scope> =
    Rc<dyn Fn(&Element, &MountContext<'scope>) -> SilexResult<()> + 'scope>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrPhase {
    Staging,
    Commit,
}

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
pub struct CombinedClasses<'scope> {
    pub statics: Vec<Cow<'scope, str>>,
    pub toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
    pub reactives: Vec<ReactiveBindingPlan<'scope>>,
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
    pub properties: Vec<ReactiveBindingPlan<'scope>>,
    pub sheets: Vec<ReactiveBindingPlan<'scope>>,
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

    /// A single reactive binding plan before class/style consolidation.
    Reactive(ReactiveBindingPlan<'scope>),

    /// Sequence of operations
    Sequence(Vec<AttrOp<'scope>>),

    /// Custom closure execution
    Custom {
        phase: AttrPhase,
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
            Self::Reactive(plan) => f.debug_tuple("Reactive").field(plan).finish(),
            Self::Sequence(seq) => f.debug_tuple("Sequence").field(seq).finish(),
            Self::Custom { .. } => f.write_str("Custom(Rc<Fn>)"),
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
            (Self::Reactive(a), Self::Reactive(b)) => a == b,
            (Self::Sequence(a), Self::Sequence(b)) => a == b,
            (
                Self::Custom {
                    phase: left_phase,
                    callback: left,
                },
                Self::Custom {
                    phase: right_phase,
                    callback: right,
                },
            ) => left_phase == right_phase && Rc::ptr_eq(left, right),
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
        AttrOp::Reactive(ReactiveBindingPlan::class_toggle(name, rx))
    }

    pub fn reactive_classes(rx: Rx<'scope, String>) -> Self {
        AttrOp::Reactive(ReactiveBindingPlan::dynamic_classes(rx))
    }

    pub fn static_styles(styles: Vec<(Cow<'scope, str>, Cow<'scope, str>)>) -> Self {
        AttrOp::CombinedStyles(CombinedStyles {
            statics: styles,
            properties: Vec::new(),
            sheets: Vec::new(),
        })
    }

    pub fn style_property(name: Cow<'scope, str>, rx: Rx<'scope, String>) -> Self {
        AttrOp::Reactive(ReactiveBindingPlan::style_property(name, rx))
    }

    pub fn reactive_stylesheet(rx: Rx<'scope, String>) -> Self {
        AttrOp::Reactive(ReactiveBindingPlan::dynamic_style(rx))
    }

    pub fn custom(
        callback: impl Fn(&Element, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::Custom {
            phase: AttrPhase::Staging,
            callback: Rc::new(callback),
        }
    }

    pub fn custom_phase(
        phase: AttrPhase,
        callback: impl Fn(&Element, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::Custom {
            phase,
            callback: Rc::new(callback),
        }
    }

    pub fn on_commit(
        callback: impl Fn(&Element, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::custom_phase(AttrPhase::Commit, callback)
    }

    pub fn apply(self, el: &Element, context: &MountContext<'scope>) -> SilexResult<()> {
        self.apply_unchecked(el, context)
    }

    fn apply_unchecked(self, el: &Element, context: &MountContext<'scope>) -> SilexResult<()> {
        let owner = context.owner();
        let error_handler = context.error_handler();
        match self {
            AttrOp::Update(AttrUpdate { target, data }) => {
                apply_update_internal(el, target, data, &owner, error_handler)?;
            }
            AttrOp::CombinedClasses(CombinedClasses {
                statics,
                toggles,
                reactives,
            }) => {
                apply_combined_classes_internal(
                    el,
                    statics,
                    toggles,
                    reactives,
                    &owner,
                    error_handler,
                )?;
            }
            AttrOp::CombinedStyles(CombinedStyles {
                statics,
                properties,
                sheets,
            }) => {
                apply_combined_styles_internal(
                    el,
                    statics,
                    properties,
                    sheets,
                    &owner,
                    error_handler,
                )?;
            }
            AttrOp::Reactive(plan) => {
                plan.install(el, &owner, error_handler)?;
            }
            AttrOp::Sequence(ops) => {
                for op in ops {
                    op.apply_unchecked(el, context)?;
                }
            }
            AttrOp::Custom { phase, callback } => match phase {
                AttrPhase::Staging => {
                    owner.with_runtime(|| callback(el, context))??;
                }
                AttrPhase::Commit => {
                    let element = el.clone();
                    let context_for_commit = context.clone();
                    let callback = callback.clone();
                    context.on_commit(move || {
                        let owner = context_for_commit.owner();
                        owner.with_runtime(|| callback(&element, &context_for_commit))??;
                        Ok(())
                    })?;
                }
            },
            AttrOp::Noop => {}
        }
        Ok(())
    }
}

fn apply_update_internal<'scope>(
    el: &Element,
    target: ApplyTarget,
    data: AttrData<'scope>,
    owner: &MountOwnerToken<'scope>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let name = target.attr_name().to_string();
    match data {
        AttrData::StaticAttr(attr) => {
            apply_attr_with_target_internal(el, &name, target, &attr)?;
        }
        AttrData::StaticJs(value) => {
            js_sys::Reflect::set(el, &JsValue::from_str(&name), &value)
                .map_err(SilexError::fatal)?;
        }
        AttrData::ReactiveAttr(rx) => {
            let el = el.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || -> SilexResult<()> {
                    let name = target.attr_name();
                    let value = rx.get()?;
                    apply_attr_with_target_internal(&el, name, target.clone(), &value)
                }),
                error_handler,
            )?;
        }
        AttrData::ReactiveString(rx) => {
            let el = el.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || -> SilexResult<()> {
                    let name = target.attr_name();
                    let val = rx.get()?;
                    apply_attr_with_target_internal(&el, name, target.clone(), &Attr::from(val))
                }),
                error_handler,
            )?;
        }
        AttrData::ReactiveBool(rx) => {
            let el = el.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || -> SilexResult<()> {
                    let name = target.attr_name();
                    let val = rx.get()?;
                    apply_attr_with_target_internal(&el, name, target.clone(), &Attr::from(val))
                }),
                error_handler,
            )?;
        }
        AttrData::ReactiveOptionString(rx) => {
            let el = el.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || -> SilexResult<()> {
                    let name = target.attr_name();
                    let val = rx.get()?;
                    let attr = match val {
                        Some(s) => Attr::from(s),
                        None => Attr::Removed,
                    };
                    apply_attr_with_target_internal(&el, name, target.clone(), &attr)
                }),
                error_handler,
            )?;
        }
        AttrData::ReactiveJs(rx) => {
            let el = el.clone();
            owner.effect(
                EffectPhase::Normal,
                Box::new(move || -> SilexResult<()> {
                    let value = rx.get()?;
                    js_sys::Reflect::set(&el, &JsValue::from_str(&name), &value)
                        .map(|_| ())
                        .map_err(SilexError::fatal)
                }),
                error_handler,
            )?;
        }
    }
    Ok(())
}

// --- Kernel Implementation Functions for Combined Op ---

struct CombinedStylePrevious {
    properties: Vec<Option<String>>,
    sheet_keys: HashSet<String>,
}

fn apply_combined_classes_internal<'scope>(
    el: &Element,
    statics: Vec<Cow<'scope, str>>,
    toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
    reactives: Vec<ReactiveBindingPlan<'scope>>,
    owner: &MountOwnerToken<'scope>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let list = el.class_list();
    // 1. 立即应用所有静态类（非响应式，仅执行一次）
    for s in &statics {
        list.add_1(s).map_err(SilexError::fatal)?;
    }

    if toggles.is_empty() && reactives.is_empty() {
        return Ok(());
    }

    let static_tokens: HashSet<String> = statics
        .iter()
        .flat_map(|class| class.split_whitespace().map(str::to_owned))
        .collect();
    // 2. 建立单 Effect 追踪所有响应式部分
    let prev_dynamic_tokens = owner.owner_state(HashSet::<String>::new())?;
    let prev_dynamic_tokens_for_effect = prev_dynamic_tokens.clone();
    let static_tokens_for_update = static_tokens.clone();
    let static_tokens_for_cleanup = static_tokens.clone();
    let el_clone = el.clone();
    let el_for_cleanup = el.clone();
    owner.effect_with_previous(
        EffectPhase::Normal,
        Box::new(
            move |previous: Option<&HashSet<String>>| -> SilexResult<HashSet<String>> {
                let list = el_clone.class_list();

                // 先合并所有动态来源。一个 token 可能同时来自 toggle 与 reactive
                // class，只有它不再被任何动态来源提供时才能从 DOM 中移除。
                let mut new_dynamic_tokens = HashSet::new();
                for (name, plan) in &toggles {
                    if plan.bool_value()? {
                        new_dynamic_tokens.insert(name.to_string());
                    }
                }

                for plan in &reactives {
                    let value = plan.string_value()?;
                    for token in value.split_whitespace() {
                        new_dynamic_tokens.insert(token.to_string());
                    }
                }

                let previous = previous.cloned().unwrap_or_default();

                // 先添加新增加的 Class，确保样式/过渡声明（transition）无缝连接，
                // 不因无类中间态产生闪烁或动画打断。
                for token in new_dynamic_tokens.difference(&previous) {
                    list.add_1(token).map_err(SilexError::fatal)?;
                }

                // 只删除已经不再由任何动态来源提供的旧 Class；静态 Class 即使
                // 同名，也必须继续保留。
                for token in previous.difference(&new_dynamic_tokens) {
                    if !static_tokens_for_update.contains(token) {
                        list.remove_1(token).map_err(SilexError::fatal)?;
                    }
                }

                prev_dynamic_tokens_for_effect.replace(new_dynamic_tokens.clone())?;
                Ok(new_dynamic_tokens)
            },
        ),
        error_handler,
    )?;

    owner.on_cleanup(
        Box::new(move || -> SilexResult<()> {
            let dynamic_tokens = prev_dynamic_tokens.take_for_cleanup().unwrap_or_default();
            let list = el_for_cleanup.class_list();
            for token in dynamic_tokens {
                if !static_tokens_for_cleanup.contains(&token) {
                    let _ = list.remove_1(&token);
                }
            }
            Ok(())
        }),
        error_handler,
    )?;
    Ok(())
}

fn apply_combined_styles_internal<'scope>(
    el: &Element,
    statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
    properties: Vec<ReactiveBindingPlan<'scope>>,
    sheets: Vec<ReactiveBindingPlan<'scope>>,
    owner: &MountOwnerToken<'scope>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<()> {
    let style = get_style_decl(el).ok_or_else(|| {
        SilexError::fatal(SilexErrorKind::Dom(
            "element does not expose a style declaration".to_string(),
        ))
    })?;

    // 1. 立即应用所有静态样式项
    for (k, v) in &statics {
        style.set_property(k, v).map_err(SilexError::fatal)?;
    }

    let static_values: std::collections::HashMap<String, String> = statics
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();

    if properties.is_empty() && sheets.is_empty() {
        return Ok(());
    }

    // 2. 建立单 Effect 追踪所有响应式样式
    let sheet_keys = owner.owner_state(HashSet::<String>::new())?;
    let sheet_keys_for_effect = sheet_keys.clone();
    let el_clone = el.clone();
    let property_names: Vec<String> = properties
        .iter()
        .filter_map(|plan| match &plan.target {
            ReactiveBindingTarget::StyleProperty(name) => Some(name.to_string()),
            _ => None,
        })
        .collect();
    let static_values_for_effect = static_values.clone();
    let el_for_cleanup = el.clone();
    let static_values_for_cleanup = static_values;

    owner.effect_with_previous(
        EffectPhase::Normal,
        Box::new(
            move |previous: Option<&CombinedStylePrevious>| -> SilexResult<CombinedStylePrevious> {
                let style = get_style_decl(&el_clone).ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Dom(
                        "element does not expose a style declaration".to_string(),
                    ))
                })?;

                // 处理单项 Property 绑定 (仅在值发生变化时更新 DOM)
                let mut next_props = Vec::with_capacity(properties.len());
                let mut property_values = std::collections::HashMap::new();
                for (i, plan) in properties.iter().enumerate() {
                    let name = match &plan.target {
                        ReactiveBindingTarget::StyleProperty(name) => name,
                        _ => continue,
                    };
                    let val = plan.string_value()?;
                    let previous_value = previous
                        .and_then(|previous| previous.properties.get(i))
                        .and_then(Option::as_deref);
                    if previous_value != Some(val.as_str()) {
                        style.set_property(name, &val).map_err(SilexError::fatal)?;
                    }
                    property_values.insert(name.to_string(), val.clone());
                    next_props.push(Some(val));
                }

                // 处理整块响应式样式字符串 (Diff 处理)
                let mut next_sheet_keys = HashSet::new();
                if !sheets.is_empty() {
                    let sheet_strings: Vec<String> = sheets
                        .iter()
                        .map(ReactiveBindingPlan::string_value)
                        .collect::<SilexResult<_>>()?;
                    let mut new_style_map = std::collections::HashMap::new();
                    for s in &sheet_strings {
                        for (k, v) in parse_style_str(s) {
                            new_style_map.insert(k.into_owned(), v.into_owned());
                        }
                    }

                    let previous_keys = previous
                        .map(|previous| &previous.sheet_keys)
                        .cloned()
                        .unwrap_or_default();
                    let stale = previous_keys
                        .iter()
                        .filter(|key| !new_style_map.contains_key(*key))
                        .cloned()
                        .collect::<Vec<_>>();
                    for key in stale {
                        if let Some(value) = property_values
                            .get(&key)
                            .or_else(|| static_values_for_effect.get(&key))
                        {
                            style.set_property(&key, value).map_err(SilexError::fatal)?;
                        } else {
                            style.remove_property(&key).map_err(SilexError::fatal)?;
                        }
                    }

                    for (key, value) in new_style_map {
                        if !property_values.contains_key(&key) {
                            style
                                .set_property(&key, &value)
                                .map_err(SilexError::fatal)?;
                        }
                        next_sheet_keys.insert(key);
                    }
                }
                sheet_keys_for_effect.replace(next_sheet_keys.clone())?;
                Ok(CombinedStylePrevious {
                    properties: next_props,
                    sheet_keys: next_sheet_keys,
                })
            },
        ),
        error_handler,
    )?;

    owner.on_cleanup(
        Box::new(move || -> SilexResult<()> {
            if let Some(style) = get_style_decl(&el_for_cleanup) {
                let sheet_keys = sheet_keys.take_for_cleanup().unwrap_or_default();
                let mut dynamic_names: HashSet<String> = property_names.into_iter().collect();
                dynamic_names.extend(sheet_keys);
                for name in dynamic_names {
                    if let Some(value) = static_values_for_cleanup.get(&name) {
                        style
                            .set_property(&name, value)
                            .map_err(SilexError::fatal)?;
                    } else {
                        style.remove_property(&name).map_err(SilexError::fatal)?;
                    }
                }
            }
            Ok(())
        }),
        error_handler,
    )?;
    Ok(())
}

// --- Kernel Functions (Non-generic DOM operations) ---

pub(crate) fn apply_attr_internal(el: &Element, name: &str, attr: &Attr<'_>) -> SilexResult<()> {
    if name.is_empty() {
        return Ok(());
    }
    match attr {
        Attr::Removed => {
            el.remove_attribute(name).map_err(SilexError::fatal)?;
        }
        Attr::Empty => {
            el.set_attribute(name, "").map_err(SilexError::fatal)?;
        }
        Attr::String(val) => match name {
            "style" => {
                if let Some(style) = get_style_decl(el) {
                    style.set_css_text(val);
                } else {
                    return Err(SilexError::fatal(SilexErrorKind::Dom(
                        "element does not expose a style declaration".to_string(),
                    )));
                }
            }
            _ => {
                el.set_attribute(name, val).map_err(SilexError::fatal)?;
            }
        },
    }
    Ok(())
}

pub(crate) fn apply_attr_with_target_internal(
    el: &Element,
    name: &str,
    target: ApplyTarget,
    attr: &Attr<'_>,
) -> SilexResult<()> {
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
                    el.set_attribute($attr_name, "")
                        .map_err(SilexError::fatal)?;
                } else {
                    el.remove_attribute($attr_name).map_err(SilexError::fatal)?;
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
            return Ok(());
        }
    }

    apply_attr_internal(el, name, attr)
}

pub(crate) fn set_string_property_internal(
    el: &Element,
    name: &str,
    value: &str,
    is_prop: bool,
) -> SilexResult<()> {
    let target = if is_prop {
        ApplyTarget::prop(name.to_string())
    } else {
        ApplyTarget::attr(name.to_string())
    };
    apply_attr_with_target_internal(el, name, target, &Attr::from(value.to_string()))
}

pub(crate) fn apply_immediate_bool_internal(
    el: &Element,
    name: &str,
    value: bool,
    is_prop: bool,
) -> SilexResult<()> {
    let target = if is_prop {
        ApplyTarget::prop(name.to_string())
    } else {
        ApplyTarget::attr(name.to_string())
    };
    apply_attr_with_target_internal(el, name, target, &Attr::from(value))
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
