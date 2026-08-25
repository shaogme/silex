#![cfg(feature = "ssr")]

use std::rc::Rc;

use crate::{
    diagnostics::error::DomResult,
    model::{
        event::EventRecord,
        node::{DomDocument, DomNode},
    },
    runtime::{backend::DomBackend, context::DomContext},
};

mod attribute;
mod backend;
mod event;
mod serialize;
mod state;
mod tree;

use backend::SsrBackend;

pub use crate::model::event::HydrationRecord;
pub use serialize::SerializeOptions;

/// Deterministic in-memory DOM backend for server rendering and tests.
pub struct SsrDom {
    backend: Rc<SsrBackend>,
    context: DomContext,
}

impl SsrDom {
    pub fn new() -> Self {
        let backend = Rc::new(SsrBackend::new());
        let erased: Rc<dyn DomBackend> = backend.clone();
        Self {
            backend,
            context: DomContext::from_backend(erased),
        }
    }

    pub fn context(&self) -> DomContext {
        self.context.clone()
    }

    pub fn document(&self) -> DomResult<DomDocument> {
        self.context.document()
    }

    pub fn serialize(&self, options: SerializeOptions) -> DomResult<String> {
        self.serialize_node(self.document()?.node(), options)
    }

    pub fn serialize_node(&self, node: &DomNode, options: SerializeOptions) -> DomResult<String> {
        let id = self.backend.node_id(node)?;
        let state = self.backend.state.borrow();
        let mut output = String::new();
        serialize::serialize_node(&state, id, &options, None, &mut output)?;
        Ok(output)
    }

    pub fn event_records(&self) -> Vec<EventRecord> {
        self.backend.state.borrow().events.clone()
    }

    pub fn hydration_records(&self) -> Vec<HydrationRecord> {
        self.event_records()
    }
}

impl Default for SsrDom {
    fn default() -> Self {
        Self::new()
    }
}
