use crate::tree::DomElement;

/// Physical target of an attribute write. Class and style remain attributes;
/// properties are represented by `PropertyRequest` and never serialized as
/// HTML attributes by the SSR backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeTarget {
    Named(String),
    Class,
    Style,
}

impl AttributeTarget {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Named(name) => name,
            Self::Class => "class",
            Self::Style => "style",
        }
    }
}

/// Safe attribute values. There is intentionally no raw-HTML variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeValue {
    Removed,
    Empty,
    Text(String),
    ClassTokens {
        add: Vec<String>,
        remove: Vec<String>,
    },
}

impl AttributeValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }
}

/// Physical property values kept separate from serialized attributes.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Removed,
    String(String),
    Bool(bool),
    Number(f64),
}

impl PropertyValue {
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }
}

#[derive(Clone, Debug)]
pub struct AttributeRequest {
    pub element: DomElement,
    pub target: AttributeTarget,
    pub value: AttributeValue,
}

impl AttributeRequest {
    pub fn new(element: &DomElement, target: AttributeTarget, value: AttributeValue) -> Self {
        Self {
            element: element.clone(),
            target,
            value,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PropertyRequest {
    pub element: DomElement,
    pub name: String,
    pub value: PropertyValue,
}

impl PropertyRequest {
    pub fn new(element: &DomElement, name: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            element: element.clone(),
            name: name.into(),
            value,
        }
    }
}
