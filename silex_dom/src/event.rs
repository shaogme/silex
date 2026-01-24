use std::borrow::Cow;
use wasm_bindgen::JsCast;
use wasm_bindgen::convert::FromWasmAbi;

/// Trait to define the metadata for a DOM event.
///
/// This trait allows us to map a specific event type (e.g., `web_sys::MouseEvent`)
/// to an event name (e.g., "click") at the type level.
pub trait EventDescriptor: Copy + Clone + 'static {
    /// The specific web_sys event type, e.g., `web_sys::MouseEvent`.
    type EventType: FromWasmAbi + JsCast + 'static;

    /// The DOM event name, e.g., "click".
    fn name(&self) -> Cow<'static, str>;

    /// Whether this event bubbles.
    /// Default is true.
    fn bubbles(&self) -> bool {
        true
    }
}

pub mod types;
pub use types::*;

// --- Event Handling Traits ---

pub struct WithEventArg;
pub struct WithoutEventArg;

pub trait EventHandler<E, M> {
    fn into_handler(self) -> Box<dyn FnMut(E)>;
}

impl<F, E> EventHandler<E, WithEventArg> for F
where
    F: FnMut(E) + 'static,
{
    fn into_handler(self) -> Box<dyn FnMut(E)> {
        Box::new(self)
    }
}

impl<F, E> EventHandler<E, WithoutEventArg> for F
where
    F: FnMut() + 'static,
    E: 'static,
{
    fn into_handler(mut self) -> Box<dyn FnMut(E)> {
        Box::new(move |_| self())
    }
}
