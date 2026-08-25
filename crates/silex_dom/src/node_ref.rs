use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};

use crate::{
    context::DomContext,
    error::{DomError, DomResult},
    tree::{DomElement, DomNode},
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
        let node = node.ok_or_else(|| match logical_state {
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

#[cfg(test)]
mod tests {
    use super::{ClearOutcome, LogicalRefState, NodeRef};
    use crate::error::DomError;
    use crate::ssr::SsrDom;
    use crate::tree::ElementSpec;

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

    #[test]
    fn binding_tokens_only_clear_their_current_generation() {
        let dom = SsrDom::new();
        let context = dom.context();
        let first = context
            .create_element(ElementSpec::new("first"))
            .expect("first element should be created")
            .node()
            .clone();
        let second = context
            .create_element(ElementSpec::new("second"))
            .expect("second element should be created")
            .node()
            .clone();
        let reference = NodeRef::new();

        let first_binding = reference
            .bind_for_mount(first.clone())
            .expect("first binding should succeed");
        assert_eq!(reference.generation(), 1);
        assert_eq!(
            reference.logical_state().expect("state should be readable"),
            LogicalRefState::Bound { generation: 1 }
        );

        let second_binding = reference
            .bind_for_mount(second.clone())
            .expect("second binding should succeed");
        assert_eq!(reference.get().expect("get should succeed"), Some(second));
        assert_eq!(
            reference.logical_state().expect("state should be readable"),
            LogicalRefState::Replaced { generation: 2 }
        );
        assert_eq!(
            first_binding
                .clear_if_current()
                .expect("stale cleanup should be harmless"),
            ClearOutcome::AlreadyReplaced
        );
        assert!(reference.get().expect("get should succeed").is_some());
        assert_eq!(
            second_binding
                .clear_if_current()
                .expect("current cleanup should succeed"),
            ClearOutcome::Cleared
        );
        assert_eq!(
            reference.logical_state().expect("state should be readable"),
            LogicalRefState::Cleared { generation: 2 }
        );
        assert_eq!(
            second_binding
                .clear_if_current()
                .expect("repeated cleanup should be harmless"),
            ClearOutcome::AlreadyCleared
        );
    }

    #[test]
    fn resolve_and_focus_validate_binding_kind_context_and_backend() {
        let dom = SsrDom::new();
        let context = dom.context();
        let element = context
            .create_element(ElementSpec::new("button"))
            .expect("element should be created");
        let text = context.create_text("text").expect("text should be created");

        let unbound = NodeRef::new();
        assert_eq!(
            unbound
                .resolve_element(&context)
                .expect("unbound resolve should be harmless"),
            None
        );
        assert_eq!(
            unbound
                .focus(&context)
                .expect_err("unbound focus should fail"),
            DomError::NotBound
        );

        let element_ref = NodeRef::new();
        element_ref
            .set(element.node().clone())
            .expect("element binding should succeed");
        assert_eq!(
            element_ref
                .resolve_element(&context)
                .expect("element resolve should succeed"),
            Some(element.clone())
        );
        assert_eq!(
            element_ref
                .focus(&context)
                .expect_err("SSR focus should be unsupported"),
            DomError::Unsupported {
                capability: "focus"
            }
        );

        let text_ref = NodeRef::new();
        text_ref.set(text).expect("text binding should succeed");
        assert_eq!(
            text_ref
                .resolve_element(&context)
                .expect_err("text cannot resolve as an element"),
            DomError::WrongNodeKind {
                expected: "element",
                actual: "text",
            }
        );

        let other_dom = SsrDom::new();
        let other_ref = NodeRef::new();
        other_ref
            .set(element.node().clone())
            .expect("cross-context test binding should succeed");
        assert_eq!(
            other_ref
                .resolve_element(&other_dom.context())
                .expect_err("cross-context resolve should fail"),
            DomError::CrossContext {
                expected: other_dom.context().backend_id().value(),
                actual: context.backend_id().value(),
            }
        );

        element_ref.clear().expect("clear should succeed");
        assert_eq!(
            element_ref
                .focus(&context)
                .expect_err("cleared focus should fail"),
            DomError::Cleared { generation: 1 }
        );
    }
}
