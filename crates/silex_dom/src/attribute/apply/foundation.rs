use std::{borrow::Cow, rc::Rc};

use silex_core::{Rx, RxValueKind, SilexError, SilexErrorKind, SilexResult};
use wasm_bindgen::JsValue;
use web_sys::Element as WebElem;

use crate::attribute::op::{
    Attr, AttrData, AttrOp, AttrUpdate, KnownProp, apply_attr_internal,
    apply_attr_with_target_internal, apply_immediate_bool_internal, get_style_decl,
    parse_style_str, set_string_property_internal,
};
use crate::view::{ViewErrorHandler, ViewOwnerToken};

// --- Unified Apply Target Enum ---

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyTarget {
    /// Standard attributes: `id`, `href`, `src` etc.
    Attr(Cow<'static, str>),
    /// Direct DOM Property (JS object property): `value`, `checked`, `muted` etc.
    Prop(Cow<'static, str>),
    /// Known strong-typed DOM Property (fast-path)
    Known(KnownProp),
    /// Specialized `.class(...)` call
    Class,
    /// Specialized `.style(...)` call
    Style,
    /// Generic application (e.g. mixins, theme variables)
    Apply,
}

impl ApplyTarget {
    /// Factory for creating an Attribute target with canonical fast-path resolution.
    pub fn attr(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        if name == "class" {
            ApplyTarget::Class
        } else if name == "style" {
            ApplyTarget::Style
        } else if let Some(kp) = KnownProp::parse(&name) {
            ApplyTarget::Known(kp)
        } else {
            ApplyTarget::Attr(name)
        }
    }

    /// Factory for creating a Property target with canonical fast-path resolution.
    pub fn prop(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        if name == "class" {
            ApplyTarget::Class
        } else if name == "style" {
            ApplyTarget::Style
        } else if let Some(kp) = KnownProp::parse(&name) {
            ApplyTarget::Known(kp)
        } else {
            ApplyTarget::Prop(name)
        }
    }

    pub fn name(&self) -> Option<Cow<'static, str>> {
        match self {
            ApplyTarget::Attr(n) | ApplyTarget::Prop(n) => Some(n.clone()),
            ApplyTarget::Known(kp) => Some(Cow::Borrowed(kp.name())),
            ApplyTarget::Class => Some(Cow::Borrowed("class")),
            ApplyTarget::Style => Some(Cow::Borrowed("style")),
            ApplyTarget::Apply => None,
        }
    }

    pub fn attr_name(&self) -> &str {
        match self {
            ApplyTarget::Attr(n) | ApplyTarget::Prop(n) => n.as_ref(),
            ApplyTarget::Known(kp) => kp.name(),
            ApplyTarget::Class => "class",
            ApplyTarget::Style => "style",
            ApplyTarget::Apply => "",
        }
    }
}

// --- Traits ---

/// Any type that can be applied as an HTML attribute, class, or style.
/// Replaces AttributeValue, ApplyClass, ApplyStyle.
pub trait ApplyToDom<'scope> {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>;

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope>
    where
        Self: Sized + 'scope,
    {
        AttrOp::Custom(Rc::new(move |el, owner, error_handler| {
            self.apply(el, target.clone(), owner, error_handler)
        }))
    }
}

pub trait ReactiveApply<'scope> {
    fn apply_to_dom(
        rx: Rx<'scope, Self, RxValueKind>,
        el: WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized;

    fn apply_pair(
        rx: Rx<'scope, Self, RxValueKind>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let _ = (rx, key, el, target);
        Ok(())
    }

    fn into_op_reactive(
        rx: Rx<'scope, Self, RxValueKind>,
        target: ApplyTarget,
    ) -> Option<AttrOp<'scope>>
    where
        Self: Sized,
    {
        let _ = (rx, target);
        None
    }

    fn into_op_pair_reactive(
        rx: Rx<'scope, Self, RxValueKind>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp<'scope>>
    where
        Self: Sized,
    {
        let _ = (rx, key, target);
        None
    }
}

// --- Basic Traits & Static Implementations ---

impl<'scope> ApplyToDom<'scope> for AttrOp<'scope> {
    fn apply(
        &self,
        el: &WebElem,
        _target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.clone().apply(el, owner, error_handler)
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        self
    }
}

impl<'scope> ApplyToDom<'scope> for fn(&WebElem) {
    fn apply(
        &self,
        el: &WebElem,
        _target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        (self)(el);
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Custom(Rc::new(move |el, _, _| {
            self(el);
            Ok(())
        }))
    }
}

impl<'scope> ApplyToDom<'scope> for Rc<dyn Fn(&WebElem)> {
    fn apply(
        &self,
        el: &WebElem,
        _target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        (self)(el);
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Custom(Rc::new(move |el, _, _| {
            self(el);
            Ok(())
        }))
    }
}

pub(crate) fn apply_immediate_string(
    el: &WebElem,
    target: &ApplyTarget,
    value: &str,
) -> SilexResult<()> {
    match target {
        ApplyTarget::Attr(n) => set_string_property_internal(el, n, value, false)?,
        ApplyTarget::Prop(n) => set_string_property_internal(el, n, value, true)?,
        ApplyTarget::Known(kp) => {
            apply_attr_with_target_internal(
                el,
                kp.name(),
                ApplyTarget::Known(*kp),
                &Attr::from(value.to_string()),
            )?;
        }
        ApplyTarget::Class => set_string_property_internal(el, "class", value, false)?,
        ApplyTarget::Style => set_string_property_internal(el, "style", value, false)?,
        ApplyTarget::Apply => {}
    }
    Ok(())
}

pub(crate) fn apply_immediate_bool(
    el: &WebElem,
    target: &ApplyTarget,
    value: bool,
) -> SilexResult<()> {
    match target {
        ApplyTarget::Attr(name) => apply_immediate_bool_internal(el, name, value, false)?,
        ApplyTarget::Prop(name) => apply_immediate_bool_internal(el, name, value, true)?,
        ApplyTarget::Known(kp) => apply_attr_with_target_internal(
            el,
            kp.name(),
            ApplyTarget::Known(*kp),
            &Attr::from(value),
        )?,
        _ => {}
    }
    Ok(())
}

pub(crate) fn apply_static_pair(
    el: &WebElem,
    target: &ApplyTarget,
    key: &str,
    value: &str,
) -> SilexResult<()> {
    match target {
        ApplyTarget::Style => {
            let style = get_style_decl(el).ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom(
                    "element does not expose a style declaration".to_string(),
                ))
            })?;
            style.set_property(key, value).map_err(SilexError::fatal)?;
        }
        _ => {
            apply_immediate_string(el, target, value)?;
        }
    }
    Ok(())
}

pub(crate) fn apply_primitive_static_internal(
    el: &WebElem,
    target: ApplyTarget,
    value: String,
) -> SilexResult<()> {
    apply_immediate_string(el, &target, &value)
}

impl<'scope, 'a: 'scope> ApplyToDom<'scope> for &'a str {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        apply_immediate_string(el, &target, self)
    }
    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match target {
            ApplyTarget::Known(_) | ApplyTarget::Attr(_) => AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::StaticAttr(Attr::from(self)),
            }),
            ApplyTarget::Prop(_) => AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::StaticJs(JsValue::from_str(self)),
            }),
            ApplyTarget::Class => {
                AttrOp::static_classes(self.split_whitespace().map(Cow::Borrowed).collect())
            }
            ApplyTarget::Style => {
                AttrOp::static_styles(parse_style_str(self).into_iter().collect())
            }
            ApplyTarget::Apply => AttrOp::Custom(Rc::new(move |el, _, _| {
                apply_immediate_string(el, &ApplyTarget::Apply, self)
            })),
        }
    }
}

impl<'scope> ApplyToDom<'scope> for String {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        apply_immediate_string(el, &target, self)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match target {
            ApplyTarget::Known(_) | ApplyTarget::Attr(_) => AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::StaticAttr(Attr::from(self)),
            }),
            ApplyTarget::Prop(_) => AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::StaticJs(JsValue::from_str(&self)),
            }),
            ApplyTarget::Class => AttrOp::static_classes(
                self.split_whitespace()
                    .map(|s| Cow::Owned(s.to_string()))
                    .collect(),
            ),
            ApplyTarget::Style => AttrOp::static_styles(
                parse_style_str(&self)
                    .into_iter()
                    .map(|(k, v)| (k.into_owned().into(), v.into_owned().into()))
                    .collect(),
            ),
            ApplyTarget::Apply => {
                let self_clone = self;
                AttrOp::Custom(Rc::new(move |el, _, _| {
                    apply_immediate_string(el, &ApplyTarget::Apply, &self_clone)
                }))
            }
        }
    }
}

impl<'scope> ApplyToDom<'scope> for &String {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        apply_immediate_string(el, &target, self)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        self.to_string().into_op(target)
    }
}

impl<'scope, 'a: 'scope> ApplyToDom<'scope> for Cow<'a, str> {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        apply_immediate_string(el, &target, self.as_ref())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match self {
            Cow::Borrowed(s) => s.into_op(target),
            Cow::Owned(s) => s.into_op(target),
        }
    }
}

impl<'scope> ApplyToDom<'scope> for Attr<'scope> {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        if let Some(name) = target.name() {
            apply_attr_with_target_internal(el, &name, target, self)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match target {
            ApplyTarget::Known(_) | ApplyTarget::Attr(_) | ApplyTarget::Prop(_) => {
                AttrOp::Update(AttrUpdate {
                    target,
                    data: AttrData::StaticAttr(self),
                })
            }
            _ => {
                let attr = self;
                AttrOp::Custom(Rc::new(move |el, _, _| apply_attr_internal(el, "", &attr)))
            }
        }
    }
}

impl<'scope> ApplyToDom<'scope> for bool {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        apply_immediate_bool(el, &target, *self)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let attr = Attr::from(self);
        match target {
            ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_) => {
                AttrOp::Update(AttrUpdate {
                    target,
                    data: AttrData::StaticAttr(attr),
                })
            }
            _ => {
                let val = self;
                AttrOp::Custom(Rc::new(move |el, _, _| {
                    apply_immediate_bool(el, &ApplyTarget::Apply, val)
                }))
            }
        }
    }
}

impl<'scope, V: ApplyToDom<'scope> + 'scope> ApplyToDom<'scope> for Option<V> {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        if let Some(v) = self {
            v.apply(el, target, owner, error_handler)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        if let Some(v) = self {
            v.into_op(target)
        } else {
            AttrOp::Noop
        }
    }
}

impl<'scope, V: ApplyToDom<'scope> + 'scope> ApplyToDom<'scope> for Vec<V> {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        for v in self {
            v.apply(el, target.clone(), owner, error_handler)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let ops = self
            .into_iter()
            .map(|v| v.into_op(target.clone()))
            .collect();
        AttrOp::Sequence(ops)
    }
}

impl<'scope, V: ApplyToDom<'scope> + 'scope, const N: usize> ApplyToDom<'scope> for [V; N] {
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        for v in self {
            v.apply(el, target.clone(), owner, error_handler)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let ops = self
            .into_iter()
            .map(|v| v.into_op(target.clone()))
            .collect();
        AttrOp::Sequence(ops)
    }
}

macro_rules! impl_apply_to_dom_for_primitive {
    ($($t:ty),*) => {
        $(
            impl<'scope> ApplyToDom<'scope> for $t {
                fn apply(
                    &self,
                    el: &WebElem,
                    target: ApplyTarget,
                    _owner: &ViewOwnerToken<'scope>,
                    _error_handler: ViewErrorHandler<'scope>,
                ) -> SilexResult<()> {
                    apply_primitive_static_internal(el, target, self.to_string())
                }

                fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
                    self.to_string().into_op(target)
                }
            }
        )*
    };
}
impl_apply_to_dom_for_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

// --- Tuples ---

// 响应式元组归一化终点：(K, Rx<T>)
impl<'scope, K, T> ApplyToDom<'scope> for (K, Rx<'scope, T, RxValueKind>)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
    T: ReactiveApply<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let (key, rx) = self.clone();
        let el = el.clone();
        T::apply_pair(rx, key.into(), el, target, owner, error_handler)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let (key, rx) = self;
        let key_cow: Cow<'static, str> = key.into();
        if let Some(op) = T::into_op_pair_reactive(rx, key_cow.clone(), target.clone()) {
            op
        } else {
            let target_effective = if target == ApplyTarget::Apply {
                ApplyTarget::attr(key_cow.clone())
            } else {
                target
            };
            if let Some(op) = T::into_op_reactive(rx, target_effective.clone()) {
                op
            } else {
                AttrOp::Custom(Rc::new(move |el, owner, error_handler| {
                    T::apply_pair(
                        rx,
                        key_cow.clone(),
                        el.clone(),
                        target_effective.clone(),
                        owner,
                        error_handler,
                    )
                }))
            }
        }
    }
}

// 静态元组 (Key, StaticValue)
impl<'scope, K> ApplyToDom<'scope> for (K, String)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let key_cow: Cow<'static, str> = self.0.clone().into();
        apply_static_pair(el, &target, key_cow.as_ref(), &self.1)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let (key, value) = self;
        let key_cow: Cow<'static, str> = key.into();
        match target {
            ApplyTarget::Style => AttrOp::static_styles(vec![(key_cow, Cow::Owned(value))]),
            ApplyTarget::Class => AttrOp::static_class(Cow::Owned(value)),
            _ => {
                let target_effective = if target == ApplyTarget::Apply {
                    ApplyTarget::attr(key_cow)
                } else {
                    target
                };
                AttrOp::Update(AttrUpdate {
                    target: target_effective,
                    data: AttrData::StaticAttr(Attr::from(value)),
                })
            }
        }
    }
}

impl<'scope, K> ApplyToDom<'scope> for (K, &'static str)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let key_cow: Cow<'static, str> = self.0.clone().into();
        apply_static_pair(el, &target, key_cow.as_ref(), self.1)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let (key, value) = self;
        let key_cow: Cow<'static, str> = key.into();
        match target {
            ApplyTarget::Style => AttrOp::static_styles(vec![(key_cow, Cow::Borrowed(value))]),
            ApplyTarget::Class => AttrOp::static_class(Cow::Borrowed(value)),
            _ => {
                let target_effective = if target == ApplyTarget::Apply {
                    ApplyTarget::attr(key_cow)
                } else {
                    target
                };
                AttrOp::Update(AttrUpdate {
                    target: target_effective,
                    data: AttrData::StaticAttr(Attr::from(value)),
                })
            }
        }
    }
}

impl<'scope, K> ApplyToDom<'scope> for (K, bool)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        _owner: &ViewOwnerToken<'scope>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let (key, value) = self.clone();
        let key_cow: Cow<'static, str> = key.into();
        match target {
            ApplyTarget::Class => {
                let list = el.class_list();
                if value {
                    list.add_1(key_cow.as_ref()).map_err(SilexError::fatal)?;
                } else {
                    list.remove_1(key_cow.as_ref()).map_err(SilexError::fatal)?;
                }
            }
            _ => {
                apply_immediate_bool(el, &target, value)?;
            }
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let (key, value) = self;
        let key_cow: Cow<'static, str> = key.into();
        match target {
            ApplyTarget::Class => {
                if value {
                    AttrOp::static_class(key_cow)
                } else {
                    AttrOp::Noop
                }
            }
            ApplyTarget::Style => AttrOp::Noop,
            _ => {
                let target_effective = if target == ApplyTarget::Apply {
                    ApplyTarget::attr(key_cow)
                } else {
                    target
                };
                AttrOp::Update(AttrUpdate {
                    target: target_effective,
                    data: AttrData::StaticAttr(Attr::from(value)),
                })
            }
        }
    }
}

// --- Attribute Group Support (Erased Collection) ---

/// 擦除后的属性组。
/// 内部持有一组 AttrOp 指令，避免了递归泛型带来的单态化膨胀。
#[derive(Clone, Default)]
pub struct AttributeGroup<'scope>(pub Vec<AttrOp<'scope>>);

/// 创建一个擦除后的属性组。
/// 这里的逻辑是：将所有输入项立即转换为 AttrOp。
/// 默认使用 ApplyTarget::Apply 作为转换上下文。
pub fn group<'scope, I>(items: I) -> AttributeGroup<'scope>
where
    I: IntoIterator,
    I::Item: ApplyToDom<'scope> + 'scope,
{
    let ops = items
        .into_iter()
        .map(|item| item.into_op(ApplyTarget::Apply))
        .collect();
    AttributeGroup(ops)
}

impl<'scope> ApplyToDom<'scope> for AttributeGroup<'scope> {
    fn apply(
        &self,
        el: &WebElem,
        _target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        for op in &self.0 {
            op.clone().apply(el, owner, error_handler)?;
        }
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        if self.0.is_empty() {
            AttrOp::Noop
        } else if self.0.len() == 1 {
            self.0.into_iter().next().unwrap()
        } else {
            AttrOp::Sequence(self.0)
        }
    }
}
