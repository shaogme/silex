use crate::{
    diagnostics::error::{DomError, DomResult},
    model::node::DomNode,
};

use super::{
    context::DomContext,
    tree::{RangeMoveRequest, RangeRequest},
};

/// A context-bound inclusive range. It does not expose concrete backend data.
#[derive(Clone)]
pub struct DomRange {
    context: DomContext,
    parent: DomNode,
    start: DomNode,
    end: DomNode,
}

impl DomRange {
    pub(crate) fn new(context: DomContext, parent: DomNode, start: DomNode, end: DomNode) -> Self {
        Self {
            context,
            parent,
            start,
            end,
        }
    }

    pub fn nodes(&self) -> DomResult<Vec<DomNode>> {
        let children = self.context.children(&self.parent)?;
        let start = children
            .iter()
            .position(|node| node == &self.start)
            .ok_or(DomError::ReferenceNotChild)?;
        let end = children
            .iter()
            .position(|node| node == &self.end)
            .ok_or(DomError::ReferenceNotChild)?;
        if start > end {
            return Err(DomError::ParentMismatch);
        }
        Ok(children[start..=end].to_vec())
    }

    pub fn remove(&self) -> DomResult<()> {
        for node in self.nodes()? {
            self.context.remove(&node)?;
        }
        Ok(())
    }

    /// Move the complete range as one ordered DOM operation.
    pub fn move_before(&self, target_parent: &DomNode, reference: &DomNode) -> DomResult<()> {
        self.context.move_range(RangeMoveRequest {
            source: RangeRequest {
                parent: self.parent.clone(),
                start: self.start.clone(),
                end: self.end.clone(),
            },
            target_parent: target_parent.clone(),
            reference: reference.clone(),
        })
    }
}
