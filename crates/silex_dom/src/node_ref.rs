use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use crate::{
    error::{DomError, DomResult},
    tree::DomNode,
};

/// Scope-bound reference to an abstract DOM node.
pub struct NodeRef<'scope> {
    value: Rc<RefCell<Option<DomNode>>>,
    marker: PhantomData<fn(&'scope ())>,
}

impl<'scope> Clone for NodeRef<'scope> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            marker: PhantomData,
        }
    }
}

impl<'scope> NodeRef<'scope> {
    pub fn new() -> Self {
        Self {
            value: Rc::new(RefCell::new(None)),
            marker: PhantomData,
        }
    }

    pub fn get(&self) -> DomResult<Option<DomNode>> {
        self.value
            .try_borrow()
            .map(|value| value.clone())
            .map_err(|_| DomError::NodeRefBorrowed)
    }

    pub fn set(&self, node: DomNode) -> DomResult<()> {
        self.value
            .try_borrow_mut()
            .map(|mut value| *value = Some(node))
            .map_err(|_| DomError::NodeRefBorrowed)
    }

    pub fn clear(&self) -> DomResult<()> {
        self.value
            .try_borrow_mut()
            .map(|mut value| *value = None)
            .map_err(|_| DomError::NodeRefBorrowed)
    }
}

impl Default for NodeRef<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::NodeRef;
    use crate::ssr::SsrDom;

    #[test]
    fn node_ref_only_stores_opaque_nodes_and_clears() {
        let dom = SsrDom::new();
        let node = dom
            .context()
            .create_text("value")
            .expect("text should be created");
        let reference = NodeRef::new();
        reference.set(node.clone()).expect("set should succeed");
        assert_eq!(reference.get().expect("get should succeed"), Some(node));
        reference.clear().expect("clear should succeed");
        assert_eq!(reference.get().expect("get should succeed"), None);
    }
}
