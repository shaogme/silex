use crate::{
    attribute::{AttributeRequest, PropertyRequest},
    error::{DomError, DomResult},
    event::{PhysicalEventRequest, WindowEventRequest},
    host::HostResource,
    tree::{
        DomDocument, DomElement, DomNode, ElementSpec, InsertRequest, RangeMoveRequest,
        RangeRequest,
    },
};

/// Stable identity attached to every backend instance and every opaque handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BackendId(u64);

impl BackendId {
    pub(crate) fn fresh() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

/// Object-safe low-level DOM and host backend.
///
/// All values crossing this trait are backend-neutral handles or request
/// structs. A backend implementation may keep any concrete representation
/// behind those handles, but it must reject handles from another identity.
pub trait DomBackend {
    fn backend_id(&self) -> BackendId;

    fn check_node(&self, node: &DomNode) -> DomResult<()> {
        if node.backend_id() != self.backend_id() {
            return Err(DomError::CrossContext {
                expected: self.backend_id().value(),
                actual: node.backend_id().value(),
            });
        }
        Ok(())
    }

    fn document(&self) -> DomResult<DomDocument>;

    fn document_body(&self) -> DomResult<Option<DomElement>> {
        Ok(None)
    }

    fn create_element(&self, spec: &ElementSpec) -> DomResult<DomElement>;

    fn create_text(&self, value: &str) -> DomResult<DomNode>;

    fn set_text(&self, node: &DomNode, value: &str) -> DomResult<()>;

    fn create_comment(&self, value: &str) -> DomResult<DomNode>;

    fn create_fragment(&self) -> DomResult<DomNode>;

    fn append(&self, parent: &DomNode, child: &DomNode) -> DomResult<()>;

    fn insert_before(&self, request: &InsertRequest) -> DomResult<()>;

    fn remove(&self, node: &DomNode) -> DomResult<()>;

    fn parent(&self, node: &DomNode) -> DomResult<Option<DomNode>>;

    fn children(&self, node: &DomNode) -> DomResult<Vec<DomNode>>;

    fn validate_range(&self, request: &RangeRequest) -> DomResult<()>;

    fn move_range(&self, request: &RangeMoveRequest) -> DomResult<()>;

    fn set_attribute(&self, request: &AttributeRequest) -> DomResult<()>;

    fn set_property(&self, request: &PropertyRequest) -> DomResult<()>;

    fn set_style_property(
        &self,
        element: &DomElement,
        name: &str,
        value: Option<&str>,
    ) -> DomResult<()> {
        let _ = (element, name, value);
        Err(DomError::Unsupported {
            capability: "style property",
        })
    }

    fn get_attribute(&self, element: &DomElement, name: &str) -> DomResult<Option<String>> {
        let _ = (element, name);
        Err(DomError::Unsupported {
            capability: "attribute read",
        })
    }

    fn focus(&self, element: &DomElement) -> DomResult<()> {
        let _ = element;
        Err(DomError::Unsupported {
            capability: "focus",
        })
    }

    fn active_element(&self) -> DomResult<Option<DomElement>> {
        Err(DomError::Unsupported {
            capability: "active element",
        })
    }

    fn contains(&self, parent: &DomElement, child: &DomNode) -> DomResult<bool> {
        let _ = (parent, child);
        Err(DomError::Unsupported {
            capability: "contains",
        })
    }

    fn document_hidden(&self) -> DomResult<Option<bool>> {
        Ok(None)
    }

    fn listen(&self, request: &PhysicalEventRequest) -> DomResult<HostResource<'static>>;

    fn listen_window(&self, request: &WindowEventRequest) -> DomResult<HostResource<'static>> {
        let _ = request;
        Err(DomError::Unsupported {
            capability: "window event listener",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BackendId;

    #[test]
    fn backend_ids_are_unique() {
        assert_ne!(BackendId::fresh(), BackendId::fresh());
    }
}
