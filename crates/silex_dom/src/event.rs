use silex_core::SilexResult;
use std::borrow::Cow;
use wasm_bindgen::{JsCast, convert::FromWasmAbi};

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

pub trait EventHandler<'scope, E, M> {
    fn into_handler(self) -> Box<dyn FnMut(E) -> SilexResult<()> + 'scope>;
}

impl<'scope, F, E> EventHandler<'scope, E, WithEventArg> for F
where
    F: FnMut(E) -> SilexResult<()> + 'scope,
{
    fn into_handler(self) -> Box<dyn FnMut(E) -> SilexResult<()> + 'scope> {
        Box::new(self)
    }
}

impl<'scope, F, E> EventHandler<'scope, E, WithoutEventArg> for F
where
    F: FnMut() -> SilexResult<()> + 'scope,
    E: 'scope,
{
    fn into_handler(mut self) -> Box<dyn FnMut(E) -> SilexResult<()> + 'scope> {
        Box::new(move |_| self())
    }
}
