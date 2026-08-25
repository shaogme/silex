use crate::context::MountContext;
use silex_core::{ErrorHandlerInput, SilexResult};
use silex_dom::{
    diagnostics::DomError,
    model::{
        DomElement, DomNode,
        event::{DomEventBridge, EventOptions, PhysicalEventRequest},
    },
    runtime::HostResource,
};
use std::{borrow::Cow, cell::Cell, rc::Rc};

pub use silex_dom::model::event::{
    DomEvent, DomRectData, EventKind, EventSpec, MouseEventData, PointerEventData,
    WindowEventRequest,
};

pub use silex_dom::model::event::EventDescriptor;

/// 事件 handler 的参数模式。
pub struct WithEventArg;
pub struct WithoutEventArg;

pub trait EventHandler<'scope, M> {
    fn into_handler(self) -> Box<dyn FnMut(DomEvent) -> SilexResult<()> + 'scope>;
}

impl<'scope, F> EventHandler<'scope, WithEventArg> for F
where
    F: FnMut(DomEvent) -> SilexResult<()> + 'scope,
{
    fn into_handler(self) -> Box<dyn FnMut(DomEvent) -> SilexResult<()> + 'scope> {
        Box::new(self)
    }
}

impl<'scope, F> EventHandler<'scope, WithoutEventArg> for F
where
    F: FnMut() -> SilexResult<()> + 'scope,
{
    fn into_handler(mut self) -> Box<dyn FnMut(DomEvent) -> SilexResult<()> + 'scope> {
        Box::new(move |_| self())
    }
}

/// 常用事件描述符。事件 payload 统一是 backend-neutral [`DomEvent`]。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Event {
    name: &'static str,
    kind: EventKind,
}

impl Event {
    pub const fn new(name: &'static str, kind: EventKind) -> Self {
        Self { name, kind }
    }
}

impl EventDescriptor for Event {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(self.name)
    }

    fn spec(&self) -> EventSpec {
        EventSpec::new(self.name, self.kind)
    }
}

macro_rules! define_events {
    ($($name:ident : $event_name:literal => $kind:ident),* $(,)?) => {
        $( #[allow(non_upper_case_globals)] pub const $name: Event = Event::new($event_name, EventKind::$kind); )*
    };
}

define_events!(
    click: "click" => Mouse,
    dblclick: "dblclick" => Mouse,
    input: "input" => Input,
    change: "change" => Form,
    keydown: "keydown" => Keyboard,
    keyup: "keyup" => Keyboard,
    focus: "focus" => Focus,
    blur: "blur" => Focus,
    mouseenter: "mouseenter" => Mouse,
    mouseleave: "mouseleave" => Mouse,
    pointerdown: "pointerdown" => Pointer,
    pointerup: "pointerup" => Pointer,
    pointermove: "pointermove" => Pointer,
    pointercancel: "pointercancel" => Pointer,
    submit: "submit" => Form,
    wheel: "wheel" => Wheel,
);

/// 将一个 owner-bound handler 接到物理 backend listener。
pub fn bind_event<'scope, E, F, M, H>(
    context: &MountContext<'scope>,
    element: &DomElement,
    event: E,
    callback: F,
    error_handler: H,
) -> SilexResult<()>
where
    E: EventDescriptor,
    F: EventHandler<'scope, M> + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    let owner = context.owner();
    let handler = callback.into_handler();
    let sender = owner.event_sender(handler, error_handler.handler_ref())?;
    let gate = Rc::new(Cell::new(true));
    let gate_for_bridge = gate.clone();
    let bridge: Rc<dyn DomEventBridge> = Rc::new(move |event: DomEvent| {
        if !gate_for_bridge.get() {
            return Ok(());
        }
        sender
            .submit(event)
            .map(|_| ())
            .map_err(|error| DomError::Backend {
                operation: "event dispatch",
                message: format!("{error:?}"),
            })
    });
    let request = PhysicalEventRequest::new(element, event.spec())
        .with_options(EventOptions::default())
        .with_bridge(bridge);
    let resource: HostResource<'static> = context.dom().listen(request)?;
    // Register the lease first and the gate second. LocalOwnerState closes
    // cleanups in reverse order, so the callback gate closes before the
    // HostResource cancellation action removes the physical listener.
    owner.track_host_resource(resource, error_handler.handler_ref())?;
    owner.on_cleanup(
        Box::new(move || {
            gate.set(false);
            Ok(())
        }),
        error_handler.handler_ref(),
    )
}

/// 将一个 owner-bound handler 接到全局 window 事件。
pub fn bind_window_event<'scope, E, F, H>(
    context: &MountContext<'scope>,
    event: E,
    callback: F,
    error_handler: H,
) -> SilexResult<()>
where
    E: EventDescriptor,
    F: EventHandler<'scope, WithEventArg> + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    let owner = context.owner();
    let handler = callback.into_handler();
    let sender = owner.event_sender(handler, error_handler.handler_ref())?;
    let bridge: Rc<dyn DomEventBridge> = Rc::new(move |event: DomEvent| {
        sender
            .submit(event)
            .map(|_| ())
            .map_err(|error| DomError::Backend {
                operation: "window event dispatch",
                message: format!("{error:?}"),
            })
    });
    let request = WindowEventRequest::new(event.spec()).with_bridge(bridge);
    let resource: HostResource<'static> = context.dom().listen_window(request)?;
    // Window listeners currently rely on HostResource cancellation only; they
    // do not share the element event's explicit View gate.
    owner.track_host_resource(resource, error_handler.handler_ref())
}

pub fn event_target(event: &DomEvent) -> &DomNode {
    event.target()
}

#[cfg(test)]
mod tests {
    use super::{Event, EventDescriptor};
    use silex_dom::model::event::EventKind;

    #[test]
    fn descriptor_is_backend_neutral() {
        let event = Event::new("custom", EventKind::Custom);
        assert_eq!(event.name(), "custom");
        assert_eq!(event.spec().kind(), EventKind::Custom);
    }
}
