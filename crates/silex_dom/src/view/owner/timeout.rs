use super::{HostResource, MountOwnerToken};
use crate::helpers::set_timeout;
use silex_core::{ErrorHandlerInput, SilexError, SilexResult};
use std::{cell::Cell, time::Duration};
use wasm_bindgen::JsValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedTimeoutTicket(u64);

pub struct OwnedTimeout<'scope> {
    ticket: OwnedTimeoutTicket,
    resource: HostResource<'scope>,
}

impl<'scope> OwnedTimeout<'scope> {
    pub fn schedule<H>(
        owner: &MountOwnerToken<'scope>,
        task: impl FnOnce() -> SilexResult<()> + 'scope,
        duration: Duration,
        error_handler: H,
    ) -> Result<Self, JsValue>
    where
        H: ErrorHandlerInput<'scope>,
    {
        let resource = set_timeout(owner, task, duration, error_handler)?;
        Ok(Self {
            ticket: next_ticket(),
            resource,
        })
    }

    pub fn ticket(&self) -> OwnedTimeoutTicket {
        self.ticket
    }

    pub fn cancel(&self) -> Result<(), SilexError> {
        self.resource.cancel()
    }

    pub fn finish(&self) {
        self.resource.finish();
    }

    pub fn is_current(&self, ticket: OwnedTimeoutTicket) -> bool {
        self.ticket == ticket && self.resource.is_active()
    }
}

fn next_ticket() -> OwnedTimeoutTicket {
    thread_local! {
        static NEXT_TICKET: Cell<u64> = const { Cell::new(0) };
    }
    NEXT_TICKET.with(|next| {
        let ticket = next.get().wrapping_add(1).max(1);
        next.set(ticket);
        OwnedTimeoutTicket(ticket)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_timeout_tickets_are_nonzero_and_distinct() {
        let first = next_ticket();
        let second = next_ticket();

        assert_ne!(first, OwnedTimeoutTicket(0));
        assert_ne!(second, OwnedTimeoutTicket(0));
        assert_ne!(first, second);
    }
}
