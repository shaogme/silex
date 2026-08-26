use crate::events::{EventDescriptor, EventHandler, WithEventArg};
use crate::kernel::MountContext;
use silex_core::{ErrorHandlerInput, SilexResult};
use silex_dom::{
    diagnostics::DomError,
    model::{
        DomElement,
        event::{DomEvent, DomEventBridge, EventOptions, PhysicalEventRequest, WindowEventRequest},
    },
    runtime::HostResource,
};
use std::{cell::Cell, rc::Rc};
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
