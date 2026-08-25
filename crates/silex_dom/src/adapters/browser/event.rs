use std::rc::Rc;

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{
    Element, Event, HtmlElement, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
    KeyboardEvent, MouseEvent, PointerEvent,
};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::{
        event::{
            DomEvent, DomEventBridge, DomEventControl, DomRectData, EventSpec, MouseEventData,
            PointerEventData,
        },
        node::DomNode,
    },
};

use super::backend::BrowserBackend;

struct BrowserEventControl {
    event: Event,
}

impl DomEventControl for BrowserEventControl {
    fn prevent_default(&self) {
        self.event.prevent_default();
    }

    fn mouse_data(&self) -> Option<MouseEventData> {
        let event = self.event.dyn_ref::<MouseEvent>()?;
        Some(MouseEventData::new(
            event.button(),
            event.ctrl_key(),
            event.meta_key(),
            event.shift_key(),
            event.alt_key(),
        ))
    }

    fn input_value(&self) -> Option<String> {
        let target = self.event.target()?;
        if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
            return Some(input.value());
        }
        if let Some(input) = target.dyn_ref::<HtmlTextAreaElement>() {
            return Some(input.value());
        }
        target
            .dyn_ref::<HtmlSelectElement>()
            .map(HtmlSelectElement::value)
    }

    fn key(&self) -> Option<String> {
        self.event
            .dyn_ref::<KeyboardEvent>()
            .map(KeyboardEvent::key)
    }

    fn pointer_data(&self) -> Option<PointerEventData> {
        let event = self.event.dyn_ref::<PointerEvent>()?;
        Some(PointerEventData::new(
            event.client_x() as f64,
            event.client_y() as f64,
            event.pointer_id(),
        ))
    }

    fn rect(&self) -> Option<DomRectData> {
        // A bubbling event may originate from a dynamically sized child. Layout
        // calculations for the listener must use the listener's own element.
        let target = self
            .event
            .current_target()
            .or_else(|| self.event.target())?;
        let element = target.dyn_into::<Element>().ok()?;
        let rect = element.get_bounding_client_rect();
        Some(DomRectData::new(
            rect.top(),
            rect.left(),
            rect.width(),
            rect.height(),
        ))
    }

    fn focus_target(&self) -> DomResult<()> {
        let target = self
            .event
            .current_target()
            .or_else(|| self.event.target())
            .ok_or(DomError::Unsupported {
                capability: "focus",
            })?;
        target
            .dyn_into::<HtmlElement>()
            .map_err(|_| DomError::Unsupported {
                capability: "focus",
            })?
            .focus()
            .map_err(|error| BrowserBackend::error("focus", error))
    }
}

pub(super) fn callback(
    spec: EventSpec,
    target_node: DomNode,
    bridge: Option<Rc<dyn DomEventBridge>>,
) -> Closure<dyn FnMut(Event)> {
    Closure::wrap_assert_unwind_safe(Box::new(move |event: Event| {
        if let Some(bridge) = &bridge {
            let control = Rc::new(BrowserEventControl {
                event: event.clone(),
            });
            let _ = bridge.dispatch(DomEvent::new_with_control(
                spec.clone(),
                target_node.clone(),
                Some(control),
            ));
        }
    }))
}
