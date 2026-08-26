use super::block::RowState;
use crate::kernel::MountTarget;
use silex_core::{SilexError, SilexResult};
use silex_dom::{
    diagnostics::DomError,
    model::{DomNode, NodeKind},
    runtime::{DomContext, DomRange, InsertRequest, RangeRequest},
};

#[derive(Clone)]
pub(crate) struct RangeHandle {
    pub(crate) context: DomContext,
    pub(crate) start: DomNode,
    pub(crate) end: DomNode,
}

pub(crate) struct RangeGuard(Option<RangeHandle>);

impl RangeGuard {
    fn new(range: RangeHandle) -> Self {
        Self(Some(range))
    }

    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for RangeGuard {
    fn drop(&mut self) {
        if let Some(range) = self.0.take() {
            let _ = range.remove();
        }
    }
}

impl RangeHandle {
    pub(crate) fn detached(context: &DomContext, label: &str) -> SilexResult<Self> {
        let fragment = context.create_fragment()?;
        Self::append(context, &fragment, label)
    }

    pub(crate) fn append(context: &DomContext, parent: &DomNode, label: &str) -> SilexResult<Self> {
        let start = context.create_comment(format!("{label}-start"))?;
        let end = context.create_comment(format!("{label}-end"))?;
        context.append(parent, &start)?;
        if let Err(error) = context.append(parent, &end) {
            let _ = context.remove(&start);
            return Err(error.into());
        }
        Ok(Self {
            context: context.clone(),
            start,
            end,
        })
    }

    pub(crate) fn at_target(target: &MountTarget, label: &str) -> SilexResult<Self> {
        match target {
            MountTarget::Append { context, parent } => Self::append(context, parent, label),
            MountTarget::Before { context, reference } => Self::before(context, reference, label),
        }
    }

    pub(crate) fn before(
        context: &DomContext,
        reference: &DomNode,
        label: &str,
    ) -> SilexResult<Self> {
        let parent = context
            .parent(reference)?
            .ok_or_else(|| SilexError::from(DomError::NoParent))?;
        let start = context.create_comment(format!("{label}-start"))?;
        let end = context.create_comment(format!("{label}-end"))?;
        context.insert_before(InsertRequest::before(&parent, &start, reference))?;
        if let Err(error) = context.insert_before(InsertRequest::before(&parent, &end, reference)) {
            let _ = context.remove(&start);
            return Err(error.into());
        }
        Ok(Self {
            context: context.clone(),
            start,
            end,
        })
    }

    pub(crate) fn parent(&self) -> SilexResult<DomNode> {
        self.context
            .parent(&self.start)?
            .ok_or_else(|| SilexError::from(DomError::NoParent))
    }

    pub(crate) fn nodes(&self) -> SilexResult<Vec<DomNode>> {
        let parent = self.parent()?;
        self.context
            .range(RangeRequest {
                parent,
                start: self.start.clone(),
                end: self.end.clone(),
            })?
            .nodes()
            .map_err(Into::into)
    }

    pub(crate) fn dom_range(&self) -> SilexResult<DomRange> {
        let parent = self.parent()?;
        self.context
            .range(RangeRequest {
                parent,
                start: self.start.clone(),
                end: self.end.clone(),
            })
            .map_err(Into::into)
    }

    pub(crate) fn remove(&self) -> SilexResult<()> {
        if self.context.parent(&self.start)?.is_none() {
            return Ok(());
        }
        self.dom_range()?.remove().map_err(Into::into)
    }

    pub(crate) fn initial_state(&self) -> SilexResult<RowState> {
        let parent = self.parent()?;
        if parent.kind() == NodeKind::Fragment {
            Ok(RowState::Detached)
        } else {
            Ok(RowState::Mounted)
        }
    }

    pub(crate) fn guard(self) -> RangeGuard {
        RangeGuard::new(self)
    }

    pub(crate) fn disarm_guard(guard: &mut RangeGuard) {
        guard.disarm();
    }
}
