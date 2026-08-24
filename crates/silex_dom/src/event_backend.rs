use std::{borrow::Cow, rc::Rc};

use crate::{
    error::{DomError, DomResult},
    tree::{DomElement, DomNode},
};

/// Broad event category used by browser adapters and SSR records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EventKind {
    Mouse,
    Keyboard,
    Input,
    Focus,
    Pointer,
    Form,
    Touch,
    Drag,
    Wheel,
    Animation,
    Composition,
    #[default]
    Custom,
}

/// Backend-neutral event metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventSpec {
    name: Cow<'static, str>,
    kind: EventKind,
    bubbles: bool,
    cancelable: bool,
}

impl EventSpec {
    pub fn new(name: impl Into<Cow<'static, str>>, kind: EventKind) -> Self {
        Self {
            name: name.into(),
            kind,
            bubbles: true,
            cancelable: true,
        }
    }

    pub fn with_flags(mut self, bubbles: bool, cancelable: bool) -> Self {
        self.bubbles = bubbles;
        self.cancelable = cancelable;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> EventKind {
        self.kind
    }

    pub fn bubbles(&self) -> bool {
        self.bubbles
    }

    pub fn cancelable(&self) -> bool {
        self.cancelable
    }
}

/// Backend-neutral physical event descriptor.
///
/// Typed browser payloads belong to the legacy View compatibility layer in
/// `event.rs`; this trait deliberately has no associated browser type.
pub trait EventDescriptor: Copy + Clone + 'static {
    fn name(&self) -> Cow<'static, str>;

    fn spec(&self) -> EventSpec {
        EventSpec::new(self.name(), EventKind::Custom)
    }

    fn bubbles(&self) -> bool {
        self.spec().bubbles()
    }
}

/// Mouse-specific fields exposed without leaking a browser event type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseEventData {
    button: i16,
    ctrl: bool,
    meta: bool,
    shift: bool,
    alt: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerEventData {
    client_x: f64,
    client_y: f64,
    pointer_id: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DomRectData {
    top: f64,
    left: f64,
    width: f64,
    height: f64,
}

impl DomRectData {
    pub const fn new(top: f64, left: f64, width: f64, height: f64) -> Self {
        Self {
            top,
            left,
            width,
            height,
        }
    }

    pub const fn top(self) -> f64 {
        self.top
    }

    pub const fn left(self) -> f64 {
        self.left
    }

    pub const fn width(self) -> f64 {
        self.width
    }

    pub const fn height(self) -> f64 {
        self.height
    }
}

impl PointerEventData {
    pub const fn new(client_x: f64, client_y: f64, pointer_id: i32) -> Self {
        Self {
            client_x,
            client_y,
            pointer_id,
        }
    }

    pub const fn client_x(self) -> f64 {
        self.client_x
    }

    pub const fn client_y(self) -> f64 {
        self.client_y
    }

    pub const fn pointer_id(self) -> i32 {
        self.pointer_id
    }
}

impl MouseEventData {
    pub const fn new(button: i16, ctrl: bool, meta: bool, shift: bool, alt: bool) -> Self {
        Self {
            button,
            ctrl,
            meta,
            shift,
            alt,
        }
    }

    pub const fn button(self) -> i16 {
        self.button
    }

    pub const fn ctrl(self) -> bool {
        self.ctrl
    }

    pub const fn meta(self) -> bool {
        self.meta
    }

    pub const fn shift(self) -> bool {
        self.shift
    }

    pub const fn alt(self) -> bool {
        self.alt
    }
}

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

impl std::fmt::Debug for DomEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Debug for WindowEventRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Debug for PhysicalEventRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

/// Backend-neutral event payload retained by SSR for hydration planning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRecord {
    pub target_backend: u64,
    pub target_identity: u64,
    pub target_kind: &'static str,
    pub spec: EventSpec,
}

/// SSR hydration metadata. Event handlers are intentionally omitted from HTML;
/// this record lets a browser adapter attach them after hydration.
pub type HydrationRecord = EventRecord;
