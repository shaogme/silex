use silex_core::{SilexError, SilexResult};
use silex_dom::{
    diagnostics::DomError,
    model::{DomElement, DomNode},
    runtime::{DomContext, InsertRequest},
};
use std::rc::Rc;

/// View 写入的物理目标。
#[derive(Clone)]
pub enum MountTarget {
    Append {
        context: DomContext,
        parent: DomNode,
    },
    Before {
        context: DomContext,
        reference: DomNode,
    },
}

impl MountTarget {
    pub fn append(context: DomContext, parent: DomNode) -> Self {
        Self::Append { context, parent }
    }

    pub fn before(context: DomContext, reference: DomNode) -> Self {
        Self::Before { context, reference }
    }

    pub fn context(&self) -> &DomContext {
        match self {
            Self::Append { context, .. } | Self::Before { context, .. } => context,
        }
    }

    pub fn append_node(&self, node: &DomNode) -> SilexResult<()> {
        match self {
            Self::Append { context, parent } => context.append(parent, node).map_err(Into::into),
            Self::Before { context, reference } => {
                let parent = context
                    .parent(reference)
                    .map_err(SilexError::from)?
                    .ok_or(DomError::NoParent)?;
                context
                    .insert_before(InsertRequest::before(&parent, node, reference))
                    .map_err(Into::into)
            }
        }
    }

    pub fn parent(&self) -> SilexResult<DomNode> {
        match self {
            Self::Append { parent, .. } => Ok(parent.clone()),
            Self::Before { context, reference } => context
                .parent(reference)
                .map_err(SilexError::from)?
                .ok_or(DomError::NoParent)
                .map_err(Into::into),
        }
    }
}

struct AncestryLink {
    element: DomElement,
    parent: Option<Rc<AncestryLink>>,
}

/// 与物理 parent chain 分离的逻辑 element ancestry。
#[derive(Clone, Default)]
pub struct MountAncestry {
    current: Option<Rc<AncestryLink>>,
}

impl MountAncestry {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn push(&self, element: &DomElement) -> Self {
        Self {
            current: Some(Rc::new(AncestryLink {
                element: element.clone(),
                parent: self.current.clone(),
            })),
        }
    }

    pub fn current_element(&self) -> Option<DomElement> {
        self.current.as_ref().map(|link| link.element.clone())
    }

    pub fn find_element<F>(&self, mut predicate: F) -> Option<DomElement>
    where
        F: FnMut(&DomElement) -> bool,
    {
        let mut current = self.current.clone();
        while let Some(link) = current {
            if predicate(&link.element) {
                return Some(link.element.clone());
            }
            current = link.parent.clone();
        }
        None
    }

    pub fn closest_logical_element(&self, _selector: &str) -> SilexResult<Option<DomElement>> {
        Err(SilexError::from(DomError::Unsupported {
            capability: "logical selector matching",
        }))
    }
}
