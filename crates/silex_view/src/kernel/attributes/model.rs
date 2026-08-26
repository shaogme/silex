use super::binding::ReactiveBindingPlan;
use silex_core::Rx;
use std::{borrow::Cow, fmt};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyTarget {
    Attr(Cow<'static, str>),
    Prop(Cow<'static, str>),
    Known(KnownProp),
    Class,
    Style,
    Apply,
}

impl ApplyTarget {
    pub fn attr(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        match name.as_ref() {
            "class" => Self::Class,
            "style" => Self::Style,
            _ => KnownProp::parse(name.as_ref()).map_or(Self::Attr(name), Self::Known),
        }
    }

    pub fn prop(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        match name.as_ref() {
            "class" => Self::Class,
            "style" => Self::Style,
            _ => KnownProp::parse(name.as_ref()).map_or(Self::Prop(name), Self::Known),
        }
    }

    pub fn name(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::Attr(name) | Self::Prop(name) => Some(name.clone()),
            Self::Known(prop) => Some(Cow::Borrowed(prop.name())),
            Self::Class => Some(Cow::Borrowed("class")),
            Self::Style => Some(Cow::Borrowed("style")),
            Self::Apply => None,
        }
    }

    pub fn attr_name(&self) -> &str {
        match self {
            Self::Attr(name) | Self::Prop(name) => name,
            Self::Known(prop) => prop.name(),
            Self::Class => "class",
            Self::Style => "style",
            Self::Apply => "",
        }
    }
}

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
            Self::Value => "value",
            Self::Checked => "checked",
            Self::Disabled => "disabled",
            Self::ReadOnly => "readOnly",
            Self::Required => "required",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "value" => Some(Self::Value),
            "checked" => Some(Self::Checked),
            "disabled" => Some(Self::Disabled),
            "readOnly" | "readonly" => Some(Self::ReadOnly),
            "required" => Some(Self::Required),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attr<'scope> {
    Removed,
    Empty,
    String(Cow<'scope, str>),
}

impl From<bool> for Attr<'_> {
    fn from(value: bool) -> Self {
        if value { Self::Empty } else { Self::Removed }
    }
}
impl<'a> From<&'a str> for Attr<'a> {
    fn from(value: &'a str) -> Self {
        if value.is_empty() {
            Self::Empty
        } else {
            Self::String(Cow::Borrowed(value))
        }
    }
}
impl From<String> for Attr<'_> {
    fn from(value: String) -> Self {
        if value.is_empty() {
            Self::Empty
        } else {
            Self::String(Cow::Owned(value))
        }
    }
}
impl<'scope> From<Cow<'scope, str>> for Attr<'scope> {
    fn from(value: Cow<'scope, str>) -> Self {
        if value.is_empty() {
            Self::Empty
        } else {
            Self::String(value)
        }
    }
}
impl<'scope, T: Into<Attr<'scope>>> From<Option<T>> for Attr<'scope> {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Removed, Into::into)
    }
}

#[derive(Clone)]
pub enum AttrData<'scope> {
    StaticAttr(Attr<'scope>),
    ReactiveAttr(Rx<'scope, Attr<'scope>>),
    ReactiveString(Rx<'scope, String>),
    ReactiveBool(Rx<'scope, bool>),
    ReactiveOptionString(Rx<'scope, Option<String>>),
}

impl fmt::Debug for AttrData<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaticAttr(value) => formatter.debug_tuple("StaticAttr").field(value).finish(),
            Self::ReactiveAttr(_) => formatter.write_str("ReactiveAttr(Rx)"),
            Self::ReactiveString(_) => formatter.write_str("ReactiveString(Rx)"),
            Self::ReactiveBool(_) => formatter.write_str("ReactiveBool(Rx)"),
            Self::ReactiveOptionString(_) => formatter.write_str("ReactiveOptionString(Rx)"),
        }
    }
}

impl PartialEq for AttrData<'_> {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (Self::StaticAttr(left), Self::StaticAttr(right)) if left == right)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttrUpdate<'scope> {
    target: ApplyTarget,
    data: AttrData<'scope>,
}

impl<'scope> AttrUpdate<'scope> {
    pub(crate) fn new(target: ApplyTarget, data: AttrData<'scope>) -> Self {
        Self { target, data }
    }

    pub(crate) fn into_parts(self) -> (ApplyTarget, AttrData<'scope>) {
        (self.target, self.data)
    }
}

#[derive(Clone)]
pub struct CombinedClasses<'scope> {
    statics: Vec<Cow<'scope, str>>,
    toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
    reactives: Vec<ReactiveBindingPlan<'scope>>,
}

impl<'scope> CombinedClasses<'scope> {
    pub fn new(
        statics: Vec<Cow<'scope, str>>,
        toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
        reactives: Vec<ReactiveBindingPlan<'scope>>,
    ) -> Self {
        Self {
            statics,
            toggles,
            reactives,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<Cow<'scope, str>>,
        Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
        Vec<ReactiveBindingPlan<'scope>>,
    ) {
        (self.statics, self.toggles, self.reactives)
    }
}

impl PartialEq for CombinedClasses<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.toggles == other.toggles
            && self.reactives == other.reactives
    }
}
impl fmt::Debug for CombinedClasses<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CombinedClasses")
            .field("statics", &self.statics)
            .field("toggles", &self.toggles.len())
            .field("reactives", &self.reactives.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct CombinedStyles<'scope> {
    statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
    properties: Vec<ReactiveBindingPlan<'scope>>,
    sheets: Vec<ReactiveBindingPlan<'scope>>,
}

impl<'scope> CombinedStyles<'scope> {
    pub fn new(
        statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
        properties: Vec<ReactiveBindingPlan<'scope>>,
        sheets: Vec<ReactiveBindingPlan<'scope>>,
    ) -> Self {
        Self {
            statics,
            properties,
            sheets,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
        Vec<ReactiveBindingPlan<'scope>>,
        Vec<ReactiveBindingPlan<'scope>>,
    ) {
        (self.statics, self.properties, self.sheets)
    }
}

impl PartialEq for CombinedStyles<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.statics == other.statics
            && self.properties == other.properties
            && self.sheets == other.sheets
    }
}
impl fmt::Debug for CombinedStyles<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CombinedStyles")
            .field("statics", &self.statics)
            .field("properties", &self.properties.len())
            .field("sheets", &self.sheets.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrPhase {
    Staging,
    Commit,
}
