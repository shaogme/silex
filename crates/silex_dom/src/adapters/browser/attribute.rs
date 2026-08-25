use js_sys::Reflect;
use wasm_bindgen::JsValue;

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::attribute::{AttributeRequest, AttributeValue, PropertyRequest, PropertyValue},
    model::node::DomElement,
};

use super::backend::BrowserBackend;

pub(super) fn set_attribute(backend: &BrowserBackend, request: &AttributeRequest) -> DomResult<()> {
    let element = backend.element(request.element.node())?;
    let name = request.target.name();
    if name.is_empty() {
        return Err(DomError::AttributeNameEmpty);
    }
    match &request.value {
        AttributeValue::Removed => element
            .remove_attribute(name)
            .map_err(|error| BrowserBackend::error("remove_attribute", error)),
        AttributeValue::Empty => element
            .set_attribute(name, "")
            .map_err(|error| BrowserBackend::error("set_attribute", error)),
        AttributeValue::Text(value) => element
            .set_attribute(name, value)
            .map_err(|error| BrowserBackend::error("set_attribute", error)),
        AttributeValue::ClassTokens { add, remove } => {
            let class_list = element.class_list();
            for class_name in add {
                class_list
                    .add_1(class_name)
                    .map_err(|error| BrowserBackend::error("class_list.add", error))?;
            }
            for class_name in remove {
                class_list
                    .remove_1(class_name)
                    .map_err(|error| BrowserBackend::error("class_list.remove", error))?;
            }
            Ok(())
        }
    }
}

pub(super) fn set_property(backend: &BrowserBackend, request: &PropertyRequest) -> DomResult<()> {
    let element = backend.element(request.element.node())?;
    if request.name.is_empty() {
        return Err(DomError::AttributeNameEmpty);
    }
    let value = match &request.value {
        PropertyValue::Removed => {
            return Reflect::delete_property(&element, &JsValue::from_str(&request.name))
                .map(|_| ())
                .map_err(|error| BrowserBackend::error("remove_property", error));
        }
        PropertyValue::String(value) => JsValue::from_str(value),
        PropertyValue::Bool(value) => JsValue::from_bool(*value),
        PropertyValue::Number(value) => JsValue::from_f64(*value),
    };
    Reflect::set(&element, &JsValue::from_str(&request.name), &value)
        .map(|_| ())
        .map_err(|error| BrowserBackend::error("set_property", error))
}

pub(super) fn set_style_property(
    backend: &BrowserBackend,
    element: &DomElement,
    name: &str,
    value: Option<&str>,
) -> DomResult<()> {
    if name.is_empty() {
        return Err(DomError::AttributeNameEmpty);
    }
    let style = backend.style(element)?;
    match value {
        Some(value) => style
            .set_property(name, value)
            .map_err(|error| BrowserBackend::error("set_style_property", error)),
        None => style
            .remove_property(name)
            .map(|_| ())
            .map_err(|error| BrowserBackend::error("remove_style_property", error)),
    }
}

pub(super) fn get_attribute(
    backend: &BrowserBackend,
    element: &DomElement,
    name: &str,
) -> DomResult<Option<String>> {
    Ok(backend.element(element.node())?.get_attribute(name))
}
