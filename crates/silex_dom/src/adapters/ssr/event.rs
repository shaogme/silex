use std::rc::Rc;

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::event::{EventRecord, PhysicalEventRequest},
    runtime::host::HostResource,
};

use super::backend::SsrBackend;

pub(super) fn listen(
    backend: &SsrBackend,
    request: &PhysicalEventRequest,
) -> DomResult<HostResource<'static>> {
    request.validate()?;
    let state = &mut *backend.state.borrow_mut();
    let target = backend.validate_node(state, request.target.node())?;
    let id = state.next_event_id;
    state.next_event_id = state
        .next_event_id
        .checked_add(1)
        .ok_or(DomError::Backend {
            operation: "listen",
            message: String::from("event record id exhausted"),
        })?;
    let target_kind = backend.record(state, target)?.kind.label();
    state.events.push(EventRecord {
        id,
        target_backend: backend.id.value(),
        target_identity: request.target.node().identity(),
        target_kind,
        spec: request.spec.clone(),
    });
    let events = Rc::clone(&backend.state);
    Ok(HostResource::with_cancel(move || {
        events.borrow_mut().events.retain(|record| record.id != id);
        Ok(())
    }))
}
