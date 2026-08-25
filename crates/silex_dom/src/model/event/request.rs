use std::{fmt, rc::Rc};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::{
        event::{EventSpec, bridge::DomEventBridge},
        node::handle::DomElement,
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventOptions {
    pub capture: bool,
    pub once: bool,
    pub passive: bool,
}

#[derive(Clone)]
pub struct PhysicalEventRequest {
    pub target: DomElement,
    pub spec: EventSpec,
    pub options: EventOptions,
    pub bridge: Option<Rc<dyn DomEventBridge>>,
}

#[derive(Clone)]
pub struct WindowEventRequest {
    pub spec: EventSpec,
    pub options: EventOptions,
    pub bridge: Option<Rc<dyn DomEventBridge>>,
}

impl fmt::Debug for WindowEventRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowEventRequest")
            .field("spec", &self.spec)
            .field("options", &self.options)
            .field("has_bridge", &self.bridge.is_some())
            .finish()
    }
}

impl WindowEventRequest {
    pub fn new(spec: EventSpec) -> Self {
        Self {
            spec,
            options: EventOptions::default(),
            bridge: None,
        }
    }

    pub fn with_options(mut self, options: EventOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_bridge(mut self, bridge: Rc<dyn DomEventBridge>) -> Self {
        self.bridge = Some(bridge);
        self
    }

    pub fn validate(&self) -> DomResult<()> {
        if self.spec.name().is_empty() {
            return Err(DomError::Backend {
                operation: "listen_window",
                message: String::from("event name is empty"),
            });
        }
        Ok(())
    }
}

impl fmt::Debug for PhysicalEventRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PhysicalEventRequest")
            .field("target", &self.target)
            .field("spec", &self.spec)
            .field("options", &self.options)
            .field("has_bridge", &self.bridge.is_some())
            .finish()
    }
}

impl PhysicalEventRequest {
    pub fn new(target: &DomElement, spec: EventSpec) -> Self {
        Self {
            target: target.clone(),
            spec,
            options: EventOptions::default(),
            bridge: None,
        }
    }

    pub fn with_options(mut self, options: EventOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_bridge(mut self, bridge: Rc<dyn DomEventBridge>) -> Self {
        self.bridge = Some(bridge);
        self
    }

    pub fn validate(&self) -> DomResult<()> {
        if self.spec.name().is_empty() {
            return Err(DomError::Backend {
                operation: "listen",
                message: String::from("event name is empty"),
            });
        }
        Ok(())
    }
}
