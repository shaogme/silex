use wasm_bindgen::{JsCast, JsValue};
use web_sys::{AddEventListenerOptions, EventTarget};

use crate::{
    diagnostics::error::DomResult,
    model::{
        event::{PhysicalEventRequest, WindowEventRequest},
        node::NodeKind,
    },
    runtime::host::HostResource,
};

use super::{backend::BrowserBackend, event};

fn options(capture: bool, once: bool, passive: bool) -> AddEventListenerOptions {
    let options = AddEventListenerOptions::new();
    options.set_capture(capture);
    options.set_once(once);
    options.set_passive(passive);
    options
}

pub(super) fn listen(
    backend: &BrowserBackend,
    request: &PhysicalEventRequest,
) -> DomResult<HostResource<'static>> {
    request.validate()?;
    let target: EventTarget = backend.element(request.target.node())?.into();
    let name = request.spec.name().to_string();
    let callback = event::callback(
        request.spec.clone(),
        request.target.node().clone(),
        request.bridge.clone(),
    );
    let options = options(
        request.options.capture,
        request.options.once,
        request.options.passive,
    );
    target
        .add_event_listener_with_callback_and_add_event_listener_options(
            &name,
            callback.as_ref().unchecked_ref(),
            &options,
        )
        .map_err(|error| BrowserBackend::error("listen", error))?;
    let cancel_target = target.clone();
    let cancel_name = name.clone();
    Ok(HostResource::with_cancel(move || {
        cancel_target
            .remove_event_listener_with_callback(&cancel_name, callback.as_ref().unchecked_ref())
            .map_err(|error| BrowserBackend::error("cancel_listener", error))
    }))
}

pub(super) fn listen_window(
    backend: &BrowserBackend,
    request: &WindowEventRequest,
) -> DomResult<HostResource<'static>> {
    request.validate()?;
    let window = backend.raw_document().default_view().ok_or_else(|| {
        BrowserBackend::error("listen_window", JsValue::from_str("window unavailable"))
    })?;
    let name = request.spec.name().to_string();
    let target_node = backend.handle(backend.document_node(), NodeKind::Document);
    let callback = event::callback(request.spec.clone(), target_node, request.bridge.clone());
    let options = options(
        request.options.capture,
        request.options.once,
        request.options.passive,
    );
    window
        .add_event_listener_with_callback_and_add_event_listener_options(
            &name,
            callback.as_ref().unchecked_ref(),
            &options,
        )
        .map_err(|error| BrowserBackend::error("listen_window", error))?;
    let cancel_window = window.clone();
    let cancel_name = name.clone();
    Ok(HostResource::with_cancel(move || {
        cancel_window
            .remove_event_listener_with_callback(&cancel_name, callback.as_ref().unchecked_ref())
            .map_err(|error| BrowserBackend::error("cancel_window_listener", error))
    }))
}
