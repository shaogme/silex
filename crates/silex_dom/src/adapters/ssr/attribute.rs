use std::collections::{BTreeMap, BTreeSet};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::{
        attribute::{AttributeRequest, AttributeValue, PropertyRequest, PropertyValue},
        node::{DomElement, NodeKind},
    },
};

use super::backend::SsrBackend;

pub(super) fn set_attribute(backend: &SsrBackend, request: &AttributeRequest) -> DomResult<()> {
    let mut state = backend.state.borrow_mut();
    let id = backend.validate_node(&state, request.element.node())?;
    let name = request.target.name();
    if name.is_empty() {
        return Err(DomError::AttributeNameEmpty);
    }
    let element = backend.record_mut(&mut state, id)?;
    if element.kind != NodeKind::Element {
        return Err(DomError::WrongNodeKind {
            expected: NodeKind::Element.label(),
            actual: element.kind.label(),
        });
    }
    match &request.value {
        AttributeValue::Removed => {
            element.attributes.remove(name);
        }
        AttributeValue::Empty => {
            element.attributes.insert(name.to_string(), String::new());
        }
        AttributeValue::Text(value) => {
            element.attributes.insert(name.to_string(), value.clone());
        }
        AttributeValue::ClassTokens { add, remove } => {
            let mut classes = element
                .attributes
                .get(name)
                .map_or_else(String::new, Clone::clone)
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>();
            classes.extend(add.iter().cloned());
            for class_name in remove {
                classes.remove(class_name);
            }
            let value = classes.into_iter().collect::<Vec<_>>().join(" ");
            if value.is_empty() {
                element.attributes.remove(name);
            } else {
                element.attributes.insert(name.to_string(), value);
            }
        }
    }
    Ok(())
}

pub(super) fn set_property(backend: &SsrBackend, request: &PropertyRequest) -> DomResult<()> {
    let mut state = backend.state.borrow_mut();
    let id = backend.validate_node(&state, request.element.node())?;
    if request.name.is_empty() {
        return Err(DomError::AttributeNameEmpty);
    }
    let element = backend.record_mut(&mut state, id)?;
    if element.kind != NodeKind::Element {
        return Err(DomError::WrongNodeKind {
            expected: NodeKind::Element.label(),
            actual: element.kind.label(),
        });
    }
    if request.value == PropertyValue::Removed {
        element.properties.remove(&request.name);
    } else {
        element
            .properties
            .insert(request.name.clone(), request.value.clone());
    }
    Ok(())
}

pub(super) fn set_style_property(
    backend: &SsrBackend,
    element: &DomElement,
    name: &str,
    value: Option<&str>,
) -> DomResult<()> {
    if name.is_empty() {
        return Err(DomError::AttributeNameEmpty);
    }
    let mut state = backend.state.borrow_mut();
    let id = backend.validate_node(&state, element.node())?;
    let record = backend.record_mut(&mut state, id)?;
    if record.kind != NodeKind::Element {
        return Err(DomError::WrongNodeKind {
            expected: NodeKind::Element.label(),
            actual: record.kind.label(),
        });
    }
    let current = record.attributes.get("style").cloned().unwrap_or_default();
    let mut declarations = BTreeMap::new();
    for declaration in current.split(';') {
        let Some((property, property_value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        if !property.is_empty() {
            declarations.insert(property.to_string(), property_value.trim().to_string());
        }
    }
    match value {
        Some(value) => {
            declarations.insert(name.to_string(), value.to_string());
        }
        None => {
            declarations.remove(name);
        }
    }
    if declarations.is_empty() {
        record.attributes.remove("style");
    } else {
        let style = declarations
            .into_iter()
            .map(|(property, value)| format!("{property}:{value};"))
            .collect::<String>();
        record.attributes.insert("style".to_string(), style);
    }
    Ok(())
}
