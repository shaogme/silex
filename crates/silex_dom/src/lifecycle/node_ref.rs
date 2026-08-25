use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};

use crate::{
    diagnostics::error::{DomError, DomResult},
    model::node::{DomElement, DomNode},
    runtime::DomContext,
};

/// Logical lifecycle of the current NodeRef binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalRefState {
    Unbound,
    Bound { generation: u64 },
    Replaced { generation: u64 },
    Cleared { generation: u64 },
}

/// Result of a generation-aware binding cleanup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearOutcome {
    Cleared,
    AlreadyReplaced,
    AlreadyCleared,
}

struct NodeBinding {
    node: DomNode,
    generation: u64,
}

struct BindingState {
    binding: Option<NodeBinding>,
    logical_state: LogicalRefState,
}

/// Scope-bound reference to an abstract DOM node.
pub struct NodeRef<'scope> {
    state: Rc<RefCell<BindingState>>,
    generation: Rc<Cell<u64>>,
    marker: PhantomData<&'scope ()>,
}

impl<'scope> Clone for NodeRef<'scope> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            generation: self.generation.clone(),
            marker: PhantomData,
        }
    }
}

impl<'scope> NodeRef<'scope> {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(BindingState {
                binding: None,
                logical_state: LogicalRefState::Unbound,
            })),
            generation: Rc::new(Cell::new(0)),
            marker: PhantomData,
        }
    }

    pub fn get(&self) -> DomResult<Option<DomNode>> {
        self.state
            .try_borrow()
            .map(|state| state.binding.as_ref().map(|binding| binding.node.clone()))
            .map_err(|_| DomError::NodeRefBorrowed)
    }

    pub fn set(&self, node: DomNode) -> DomResult<()> {
        let _ = self.bind_for_mount(node)?;
        Ok(())
    }

    pub fn clear(&self) -> DomResult<()> {
        self.state
            .try_borrow_mut()
            .map(|mut state| {
                if let Some(binding) = state.binding.take() {
                    state.logical_state = LogicalRefState::Cleared {
                        generation: binding.generation,
                    };
                }
            })
            .map_err(|_| DomError::NodeRefBorrowed)
    }

    /// Bind a node and return a token for generation-aware owner cleanup.
    pub fn bind_for_mount(&self, node: DomNode) -> DomResult<NodeRefBinding<'scope>> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| DomError::NodeRefBorrowed)?;
        let generation = self
            .generation
            .get()
            .checked_add(1)
            .ok_or(DomError::BindingGenerationExhausted)?;
        let replaced = state.binding.is_some();
        state.binding = Some(NodeBinding { node, generation });
        state.logical_state = if replaced {
            LogicalRefState::Replaced { generation }
        } else {
            LogicalRefState::Bound { generation }
        };
        self.generation.set(generation);
        Ok(NodeRefBinding {
            reference: self.clone(),
            generation,
        })
    }

    /// Return the latest generation allocated for this reference.
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub fn logical_state(&self) -> DomResult<LogicalRefState> {
        self.state
            .try_borrow()
            .map(|state| state.logical_state)
            .map_err(|_| DomError::NodeRefBorrowed)
    }

    /// Resolve the current opaque node through the caller-provided context.
    pub fn resolve_element(&self, context: &DomContext) -> DomResult<Option<DomElement>> {
        let node = self.get()?;
        node.map_or(Ok(None), |node| context.element(&node).map(Some))
    }

    /// Focus the current element through the caller-provided context.
    pub fn focus(&self, context: &DomContext) -> DomResult<()> {
        let (node, logical_state) = self
            .state
            .try_borrow()
            .map(|state| {
                (
                    state.binding.as_ref().map(|binding| binding.node.clone()),
                    state.logical_state,
                )
            })
            .map_err(|_| DomError::NodeRefBorrowed)?;
        let node = node.ok_or(match logical_state {
            LogicalRefState::Cleared { generation } => DomError::Cleared { generation },
            _ => DomError::NotBound,
        })?;
        let element = context.element(&node)?;
        context.focus(&element)
    }

    fn clear_generation(&self, generation: u64) -> DomResult<ClearOutcome> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| DomError::NodeRefBorrowed)?;
        match state.binding.as_ref().map(|binding| binding.generation) {
            Some(current) if current == generation => {
                state.binding = None;
                state.logical_state = LogicalRefState::Cleared { generation };
                Ok(ClearOutcome::Cleared)
            }
            Some(_) => Ok(ClearOutcome::AlreadyReplaced),
            None if state.logical_state == (LogicalRefState::Cleared { generation }) => {
                Ok(ClearOutcome::AlreadyCleared)
            }
            None => Ok(ClearOutcome::AlreadyReplaced),
        }
    }
}

/// Generation-bound cleanup handle used by View mount glue.
pub struct NodeRefBinding<'scope> {
    reference: NodeRef<'scope>,
    generation: u64,
}

impl<'scope> NodeRefBinding<'scope> {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn clear_if_current(&self) -> DomResult<ClearOutcome> {
        self.reference.clear_generation(self.generation)
    }
}

impl<'scope> Clone for NodeRefBinding<'scope> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            generation: self.generation,
        }
    }
}

impl Default for NodeRef<'_> {
    fn default() -> Self {
        Self::new()
    }
}
