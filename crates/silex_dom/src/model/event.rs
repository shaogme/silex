use std::borrow::Cow;

pub mod bridge;
pub mod request;

pub use bridge::{DomEvent, DomEventBridge, DomEventControl};
pub use request::{EventOptions, PhysicalEventRequest, WindowEventRequest};

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
/// High-level View handlers use the backend-neutral `DomEvent` bridge; this
/// trait deliberately has no associated browser type.
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

/// Backend-neutral event payload retained by SSR for hydration planning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRecord {
    pub id: u64,
    pub target_backend: u64,
    pub target_identity: u64,
    pub target_kind: &'static str,
    pub spec: EventSpec,
}

/// SSR hydration metadata. Event handlers are intentionally omitted from HTML;
/// this record lets a browser adapter attach them after hydration.
pub type HydrationRecord = EventRecord;
