use super::context::{MountContext, MountTarget, MountTransaction};
use super::dynamic::BranchRenderContext;
use super::owner::{MountErrorHandler, MountOwner, MountOwnerToken, MountState};
use crate::attribute::AttrOp;
use silex_core::{
    CloseError, ClosePhase, CloseSource, CloseTransaction, OwnerChild, ReactiveError, SilexError,
    SilexErrorKind, SilexResult,
};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use web_sys::Node;

type RowCallback<'scope, T> = Box<dyn FnMut(T, usize) + 'scope>;

struct RowUpdaterState<'scope, T> {
    generation: Cell<u64>,
    callback: RefCell<Option<RowCallback<'scope, T>>>,
}

/// Owner-bound typed update capability for a persistent row.
///
/// The controller creates this value and passes it to an opt-in stateful row
/// factory. The callback is removed before the row owner is disposed, so
/// cloned updaters become inert without retaining a scoped runtime handle.
pub struct RowUpdater<'scope, T> {
    state: Rc<RowUpdaterState<'scope, T>>,
    generation: u64,
}

impl<'scope, T> Clone for RowUpdater<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            generation: self.generation,
        }
    }
}

impl<'scope, T> RowUpdater<'scope, T> {
    fn new() -> Self {
        Self {
            state: Rc::new(RowUpdaterState {
                generation: Cell::new(0),
                callback: RefCell::new(None),
            }),
            generation: 0,
        }
    }

    /// Bind the row's typed update callback exactly once.
    pub fn bind<F>(&self, callback: F) -> bool
    where
        F: FnMut(T, usize) + 'scope,
    {
        if !self.is_generation_active() {
            return false;
        }
        let mut slot = self.state.callback.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(Box::new(callback));
        true
    }

    /// Dispatch a new item/index pair while the row is still active.
    pub fn update(&self, item: T, index: usize) -> bool {
        if !self.is_generation_active() {
            return false;
        }

        let Some(mut callback) = self.state.callback.borrow_mut().take() else {
            return false;
        };
        let result = catch_unwind(AssertUnwindSafe(|| callback(item, index)));
        if self.is_generation_active() && self.state.callback.borrow().is_none() {
            *self.state.callback.borrow_mut() = Some(callback);
        }
        match result {
            Ok(()) => true,
            Err(panic) => resume_unwind(panic),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_generation_active() && self.state.callback.borrow().is_some()
    }

    fn is_generation_active(&self) -> bool {
        self.state.generation.get() == self.generation
    }

    fn invalidate(&self) {
        if self.is_generation_active() {
            self.state.generation.set(self.generation.wrapping_add(1));
        }
        let _ = self.state.callback.borrow_mut().take();
    }
}

pub(crate) struct RowRenderContext<'scope, T> {
    pub(crate) item: T,
    pub(crate) index: usize,
    pub(crate) context: MountContext<'scope>,
    pub(crate) attrs: Vec<AttrOp<'scope>>,
    pub(crate) branch_context: Option<BranchRenderContext<'scope>>,
    pub(crate) updater: RowUpdater<'scope, T>,
}

pub(crate) struct RowRenderer<'scope, T> {
    inner: Rc<dyn Fn(RowRenderContext<'scope, T>) -> SilexResult<()> + 'scope>,
}

impl<'scope, T> Clone for RowRenderer<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'scope, T> RowRenderer<'scope, T> {
    pub(crate) fn new<F>(render: F) -> Self
    where
        F: Fn(RowRenderContext<'scope, T>) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(render),
        }
    }

    pub(crate) fn call(&self, args: RowRenderContext<'scope, T>) -> SilexResult<()> {
        (self.inner)(args)
    }
}

#[derive(Clone)]
pub(crate) struct NodeRange {
    pub(crate) start: Node,
    pub(crate) end: Node,
}

struct RangeGuard(Option<NodeRange>);

impl RangeGuard {
    fn new(range: NodeRange) -> Self {
        Self(Some(range))
    }

    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for RangeGuard {
    fn drop(&mut self) {
        if let Some(range) = self.0.take() {
            range.remove();
        }
    }
}

impl NodeRange {
    pub(crate) fn detached(label: &str) -> Result<Self, SilexError> {
        let fragment: Node = crate::document().create_document_fragment().into();
        Self::append(&fragment, label)
    }

    pub(crate) fn append(parent: &Node, label: &str) -> Result<Self, SilexError> {
        let document = crate::document();
        let start: Node = document.create_comment(&format!("{label}-start")).into();
        let end: Node = document.create_comment(&format!("{label}-end")).into();
        parent.append_child(&start).map_err(SilexError::fatal)?;
        if let Err(error) = parent.append_child(&end).map_err(SilexError::fatal) {
            let _ = parent.remove_child(&start);
            return Err(error);
        }
        Ok(Self { start, end })
    }

    pub(crate) fn at_target(target: &MountTarget, label: &str) -> Result<Self, SilexError> {
        match target {
            MountTarget::Append(parent) => Self::append(parent, label),
            MountTarget::Before(reference) => Self::before(reference, label),
        }
    }

    pub(crate) fn before(reference: &Node, label: &str) -> Result<Self, SilexError> {
        let Some(parent) = reference.parent_node() else {
            return Err(SilexError::fatal(SilexErrorKind::Dom(
                "cannot create a row range without a parent".to_string(),
            )));
        };
        let document = crate::document();
        let start: Node = document.create_comment(&format!("{label}-start")).into();
        let end: Node = document.create_comment(&format!("{label}-end")).into();
        parent
            .insert_before(&start, Some(reference))
            .map_err(SilexError::fatal)?;
        if let Err(error) = parent
            .insert_before(&end, Some(reference))
            .map_err(SilexError::fatal)
        {
            let _ = parent.remove_child(&start);
            return Err(error);
        }
        Ok(Self { start, end })
    }

    pub(crate) fn clear(&self) {
        let Some(parent) = self.start.parent_node() else {
            return;
        };
        while let Some(node) = self.start.next_sibling() {
            if node == self.end {
                break;
            }
            let _ = parent.remove_child(&node);
        }
    }

    pub(crate) fn remove(&self) {
        self.clear();
        if let Some(parent) = self.start.parent_node() {
            let _ = parent.remove_child(&self.start);
        }
        if let Some(parent) = self.end.parent_node() {
            let _ = parent.remove_child(&self.end);
        }
    }

    pub(crate) fn append_to(&self, target: &Node) -> SilexResult<()> {
        let Some(source_parent) = self.start.parent_node() else {
            return Err(SilexError::fatal(SilexErrorKind::Dom(
                "cannot move a detached row range".to_string(),
            )));
        };
        if self.end.parent_node().as_ref() != Some(&source_parent) {
            return Err(SilexError::fatal(SilexErrorKind::Dom(
                "cannot move an incomplete row range".to_string(),
            )));
        }

        let mut nodes = Vec::new();
        let mut current = Some(self.start.clone());
        while let Some(node) = current {
            let next = node.next_sibling();
            let is_end = node == self.end;
            nodes.push(node);
            if is_end {
                break;
            }
            current = next;
        }
        if nodes.last().is_none_or(|node| *node != self.end) {
            return Err(SilexError::fatal(SilexErrorKind::Dom(
                "cannot move an incomplete row range".to_string(),
            )));
        }
        for node in nodes {
            target.append_child(&node).map_err(SilexError::fatal)?;
        }
        Ok(())
    }
}

pub(crate) struct RowInstance<'scope, T> {
    range: NodeRange,
    row_scope: MountOwnerToken<'scope>,
    content_owner: Option<MountOwnerToken<'scope>>,
    runtime_child: Option<OwnerChild<'scope>>,
    render_scope: Option<MountOwnerToken<'scope>>,
    render_content_scope: Option<MountState<'scope, Option<MountOwnerToken<'scope>>>>,
    render_nodes: Option<MountState<'scope, Vec<Node>>>,
    render: RowRenderer<'scope, T>,
    attrs: Vec<AttrOp<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    context: MountContext<'scope>,
    updater: RowUpdater<'scope, T>,
    stateful: bool,
    active: Cell<bool>,
    generation: u64,
    item: T,
    index: usize,
    marker: PhantomData<fn(T)>,
}

pub(crate) struct RowInstanceConfig<'scope, T> {
    pub(crate) range: NodeRange,
    pub(crate) render: RowRenderer<'scope, T>,
    pub(crate) attrs: Vec<AttrOp<'scope>>,
    pub(crate) item: T,
    pub(crate) index: usize,
    pub(crate) stateful: bool,
    pub(crate) branch_runtime: bool,
    pub(crate) error_handler: MountErrorHandler<'scope>,
    pub(crate) context: MountContext<'scope>,
}

impl<'scope, T: Clone + 'scope> RowInstance<'scope, T> {
    pub(crate) fn new(
        owner: &dyn MountOwner<'scope>,
        config: RowInstanceConfig<'scope, T>,
    ) -> SilexResult<Self> {
        let RowInstanceConfig {
            range,
            render,
            attrs,
            item,
            index,
            stateful,
            branch_runtime,
            error_handler,
            context,
        } = config;
        let mut range_guard = RangeGuard::new(range.clone());
        let updater = RowUpdater::new();
        let row_scope = owner.child();
        let (runtime_child, content_owner) = if branch_runtime {
            let (child, content_owner) = row_scope.branch_child()?;
            (Some(child), Some(content_owner))
        } else {
            (None, None)
        };
        let mut controller = Self {
            range,
            row_scope,
            content_owner,
            runtime_child,
            render_scope: None,
            render_content_scope: None,
            render_nodes: None,
            render,
            attrs,
            error_handler,
            context,
            updater,
            stateful,
            active: Cell::new(true),
            generation: 0,
            item: item.clone(),
            index,
            marker: PhantomData,
        };
        if let Err(error) = controller.mount_render(item, index) {
            if let Err(close_error) = controller.dispose() {
                controller.row_scope.report_close_error(close_error);
            }
            return Err(error);
        }
        if stateful && !controller.updater.is_active() {
            if let Err(close_error) = controller.dispose() {
                controller.row_scope.report_close_error(close_error);
            }
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "stateful row updater was not bound during initial render".to_string(),
            )));
        }
        range_guard.disarm();
        Ok(controller)
    }

    pub(crate) fn update(&mut self, item: T, index: usize) -> SilexResult<()> {
        if !self.active.get() {
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        let next_item = item.clone();
        let result = if self.stateful {
            if self.updater.update(item, index) {
                Ok(())
            } else {
                Err(SilexError::fatal(SilexErrorKind::Framework(
                    "stateful row updater rejected update".to_string(),
                )))
            }
        } else {
            self.mount_render(item, index)
        };
        if result.is_ok() {
            self.item = next_item;
            self.index = index;
        }
        result
    }

    pub(crate) fn snapshot(&self) -> (T, usize) {
        (self.item.clone(), self.index)
    }

    fn mount_render(&mut self, item: T, index: usize) -> SilexResult<()> {
        let previous_scope = self.render_scope.take();
        let previous_content_scope = self.render_content_scope.take();
        let previous_nodes = self
            .render_nodes
            .as_ref()
            .and_then(|nodes| nodes.with(Clone::clone).ok())
            .unwrap_or_default();
        let render_scope = self.row_scope.child();
        let rendered_nodes = render_scope.owner_state(previous_nodes)?;
        let rendered_scope = render_scope.owner_state(None::<MountOwnerToken<'scope>>)?;
        let row_scope = self.row_scope.clone();
        let content_owner = self.content_owner.clone();
        let range = self.range.clone();
        let render = self.render.clone();
        let attrs = self.attrs.clone();
        let updater = self.updater.clone();
        let rendered_nodes_for_effect = rendered_nodes.clone();
        let rendered_scope_for_effect = rendered_scope.clone();
        let error_handler = self.error_handler;
        let document = crate::document();
        let render_handler = self.error_handler;
        let base_context = self.context.clone();
        let registration = catch_unwind(AssertUnwindSafe(|| {
            render_scope.effect(
                Box::new(move || -> SilexResult<()> {
                    let old_nodes = rendered_nodes_for_effect.with(Clone::clone)?;
                    let candidate_scope = content_owner
                        .as_ref()
                        .map(MountOwnerToken::child)
                        .unwrap_or_else(|| row_scope.child());
                    let candidate_token = candidate_scope.clone();
                    let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                        let fragment = document.create_document_fragment();
                        let fragment_node: Node = fragment.into();
                        let render_transaction = MountTransaction::new();
                        let render_context = base_context.with_parts(
                            MountTarget::Append(fragment_node.clone()),
                            base_context.ancestry().clone(),
                            candidate_token,
                            render_transaction.clone(),
                        );
                        render.call(RowRenderContext {
                            item: item.clone(),
                            index,
                            context: render_context,
                            attrs: attrs.clone(),
                            branch_context: content_owner.as_ref().map(|_| {
                                BranchRenderContext::new(
                                    candidate_scope.clone(),
                                    candidate_scope.runtime_access(),
                                    error_handler,
                                )
                            }),
                            updater: updater.clone(),
                        })?;
                        let new_nodes = child_nodes(&fragment_node);
                        let Some(parent) = range.end.parent_node() else {
                            return Err(SilexError::fatal(SilexErrorKind::Dom(
                                "cannot commit row render without a parent".to_string(),
                            )));
                        };
                        parent
                            .insert_before(&fragment_node, Some(&range.end))
                            .map_err(SilexError::fatal)?;
                        render_transaction.commit()?;
                        for node in old_nodes {
                            if node.parent_node().is_some() {
                                let _ = parent.remove_child(&node);
                            }
                        }
                        rendered_nodes_for_effect.replace(new_nodes)?;
                        Ok(())
                    }));
                    match result {
                        Ok(Ok(())) => {
                            let previous = rendered_scope_for_effect
                                .replace(Some(candidate_scope))?
                                .flatten();
                            if let Some(scope) = previous
                                && let Err(error) = close_scope(scope)
                            {
                                row_scope.report_close_error(error);
                            }
                            Ok(())
                        }
                        Ok(Err(error)) => {
                            if let Err(close_error) = close_scope(candidate_scope) {
                                row_scope.report_close_error(close_error);
                            }
                            Err(error)
                        }
                        Err(panic) => {
                            let error = SilexError::fatal(SilexErrorKind::Javascript(
                                panic_message(&panic, "Row render"),
                            ));
                            if let Err(close_error) = close_scope(candidate_scope) {
                                row_scope.report_close_error(close_error);
                            }
                            Err(error)
                        }
                    }
                }),
                render_handler,
            )
        }));

        let registration = match registration {
            Ok(result) => result,
            Err(panic) => {
                if let Err(close_error) = dispose_render_candidate(&rendered_scope) {
                    self.row_scope.report_close_error(close_error);
                }
                if let Err(close_error) = close_scope(render_scope) {
                    self.row_scope.report_close_error(close_error);
                }
                self.render_scope = previous_scope;
                self.render_content_scope = previous_content_scope;
                return Err(SilexError::fatal(SilexErrorKind::Javascript(
                    panic_message(&panic, "Row effect"),
                )));
            }
        };
        if let Err(error) = registration {
            if let Err(close_error) = dispose_render_candidate(&rendered_scope) {
                self.row_scope.report_close_error(close_error);
            }
            if let Err(close_error) = close_scope(render_scope) {
                self.row_scope.report_close_error(close_error);
            }
            self.render_scope = previous_scope;
            self.render_content_scope = previous_content_scope;
            return Err(error);
        }

        if let Some(scope) = previous_scope
            && let Err(close_error) = close_scope(scope)
        {
            self.row_scope.report_close_error(close_error);
        }
        if let Some(scope) = previous_content_scope
            && let Err(close_error) = dispose_render_candidate(&scope)
        {
            self.row_scope.report_close_error(close_error);
        }
        self.render_scope = Some(render_scope);
        self.render_content_scope = Some(rendered_scope);
        self.render_nodes = Some(rendered_nodes);
        Ok(())
    }
}

impl<'scope, T> RowInstance<'scope, T> {
    pub(crate) fn append_to(&self, target: &Node) -> SilexResult<()> {
        if !self.active.get() {
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        self.range.append_to(target)
    }

    pub(crate) fn dispose(&mut self) -> Result<(), CloseError> {
        if !self.active.replace(false) {
            return Ok(());
        }
        self.generation = self.generation.wrapping_add(1);
        self.updater.invalidate();
        let result = self.dispose_owners();
        self.range.remove();
        result
    }

    fn dispose_owners(&mut self) -> Result<(), CloseError> {
        let mut transaction = CloseTransaction::new();
        if let Some(scope) = self.render_scope.take()
            && let Err(error) = close_scope(scope)
        {
            transaction.push_error(ClosePhase::Effect, CloseSource::Effect, error);
        }
        if let Some(scope) = self.render_content_scope.take()
            && let Err(error) = dispose_render_candidate(&scope)
        {
            transaction.push_error(ClosePhase::Effect, CloseSource::Child, error);
        }
        if let Err(error) = close_scope(self.row_scope.clone()) {
            transaction.push_error(ClosePhase::Child, CloseSource::Owner, error);
        }
        if let Err(error) = self.close_owned_runtime() {
            transaction.push_error(ClosePhase::Runtime, CloseSource::Owner, error);
        }
        transaction.finish().map_or(Ok(()), Err)
    }

    fn close_owned_runtime(&self) -> Result<(), CloseError> {
        self.runtime_child
            .as_ref()
            .map_or(Ok(()), OwnerChild::close)
    }
}

fn close_scope<'scope>(scope: MountOwnerToken<'scope>) -> Result<(), CloseError> {
    match catch_unwind(AssertUnwindSafe(|| scope.close())) {
        Ok(result) => result,
        Err(panic) => Err(CloseError::from_panic(panic)),
    }
}

fn dispose_render_candidate<'scope>(
    scope: &MountState<'scope, Option<MountOwnerToken<'scope>>>,
) -> Result<(), CloseError> {
    if let Some(scope) = scope.take_for_cleanup().flatten() {
        close_scope(scope)
    } else {
        Ok(())
    }
}

fn child_nodes(parent: &Node) -> Vec<Node> {
    let children = parent.child_nodes();
    (0..children.length())
        .filter_map(|index| children.item(index))
        .collect()
}

impl<T> Drop for RowInstance<'_, T> {
    fn drop(&mut self) {
        if let Err(error) = self.dispose() {
            self.row_scope.report_close_error(error);
        }
    }
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>, prefix: &str) -> String {
    if let Some(value) = panic.downcast_ref::<&str>() {
        format!("{prefix}: {value}")
    } else if let Some(value) = panic.downcast_ref::<String>() {
        format!("{prefix}: {value}")
    } else {
        format!("{prefix}: unknown panic")
    }
}

#[cfg(test)]
mod tests {
    use super::RowUpdater;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn row_updater_rejects_calls_after_invalidation() {
        let calls = Rc::new(Cell::new(0));
        let calls_for_callback = calls.clone();
        let updater = RowUpdater::new();
        let stale = updater.clone();
        assert!(updater.bind(move |_, _| calls_for_callback.set(calls_for_callback.get() + 1)));
        assert!(stale.update(1, 0));
        assert_eq!(calls.get(), 1);

        updater.invalidate();

        assert!(!stale.is_active());
        assert!(!stale.update(2, 0));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn row_updater_rejects_reentrant_dispatch_without_borrow_panic() {
        let calls = Rc::new(Cell::new(0));
        let updater = RowUpdater::new();
        let reentrant = updater.clone();
        let calls_for_callback = calls.clone();
        assert!(updater.bind(move |_, _| {
            calls_for_callback.set(calls_for_callback.get() + 1);
            assert!(!reentrant.update(2, 0));
        }));

        assert!(updater.update(1, 0));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn row_updater_propagates_callback_panic_for_transaction_boundary() {
        let updater = RowUpdater::new();
        assert!(updater.bind(|_, _| panic!("intentional updater panic")));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            updater.update(1, 0);
        }));
        assert!(result.is_err());

        updater.invalidate();
        assert!(!updater.update(2, 0));
    }
}
