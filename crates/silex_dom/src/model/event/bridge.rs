use std::{fmt, rc::Rc};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::{
        event::{DomRectData, EventSpec, MouseEventData, PointerEventData},
        node::handle::DomNode,
    },
};

/// Optional controls supplied by a physical event adapter.
pub trait DomEventControl {
    fn prevent_default(&self);

    fn mouse_data(&self) -> Option<MouseEventData>;

    fn input_value(&self) -> Option<String> {
        None
    }

    fn key(&self) -> Option<String> {
        None
    }

    fn pointer_data(&self) -> Option<PointerEventData> {
        None
    }

    fn rect(&self) -> Option<DomRectData> {
        None
    }

    fn focus_target(&self) -> DomResult<()> {
        Err(DomError::Unsupported {
            capability: "focus",
        })
    }
}

/// Opaque physical event delivered by an adapter bridge.
#[derive(Clone)]
pub struct DomEvent {
    spec: EventSpec,
    target: DomNode,
    control: Option<Rc<dyn DomEventControl>>,
}

impl DomEvent {
    #[cfg(feature = "browser")]
    pub(crate) fn new_with_control(
        spec: EventSpec,
        target: DomNode,
        control: Option<Rc<dyn DomEventControl>>,
    ) -> Self {
        Self {
            spec,
            target,
            control,
        }
    }

    pub fn spec(&self) -> &EventSpec {
        &self.spec
    }

    pub fn target(&self) -> &DomNode {
        &self.target
    }

    pub fn prevent_default(&self) {
        if let Some(control) = &self.control {
            control.prevent_default();
        }
    }

    pub fn mouse_data(&self) -> Option<MouseEventData> {
        self.control
            .as_ref()
            .and_then(|control| control.mouse_data())
    }

    pub fn input_value(&self) -> Option<String> {
        self.control
            .as_ref()
            .and_then(|control| control.input_value())
    }

    pub fn key(&self) -> Option<String> {
        self.control.as_ref().and_then(|control| control.key())
    }

    pub fn pointer_data(&self) -> Option<PointerEventData> {
        self.control
            .as_ref()
            .and_then(|control| control.pointer_data())
    }

    pub fn rect(&self) -> Option<DomRectData> {
        self.control.as_ref().and_then(|control| control.rect())
    }

    pub fn focus_target(&self) -> DomResult<()> {
        self.control.as_ref().map_or_else(
            || {
                Err(DomError::Unsupported {
                    capability: "focus",
                })
            },
            |control| control.focus_target(),
        )
    }
}

impl fmt::Debug for DomEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomEvent")
            .field("spec", &self.spec)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

/// Optional framework-owned bridge. User callbacks never cross `DomBackend`.
pub trait DomEventBridge {
    fn dispatch(&self, event: DomEvent) -> DomResult<()>;
}

impl<F> DomEventBridge for F
where
    F: Fn(DomEvent) -> DomResult<()> + 'static,
{
    fn dispatch(&self, event: DomEvent) -> DomResult<()> {
        self(event)
    }
}
