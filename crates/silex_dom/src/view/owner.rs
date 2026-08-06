use crate::attribute::PendingAttribute;
use crate::view::{OwnedViewOwner, ViewOwner, ViewOwnerToken};
use silex_core::{ErrorReporter, OwnedScope, RuntimeInputs, SilexError};
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

pub(crate) struct RowRenderArgs<'scope, T> {
    pub(crate) item: T,
    pub(crate) index: usize,
    pub(crate) parent: Node,
    pub(crate) attrs: Vec<PendingAttribute<'scope>>,
    pub(crate) owner: ViewOwnerToken<'scope>,
    pub(crate) updater: RowUpdater<'scope, T>,
}

impl<'scope, T> RowRenderArgs<'scope, T> {
    pub(crate) fn new(
        item: T,
        index: usize,
        parent: Node,
        attrs: Vec<PendingAttribute<'scope>>,
        owner: ViewOwnerToken<'scope>,
        updater: RowUpdater<'scope, T>,
    ) -> Self {
        Self {
            item,
            index,
            parent,
            attrs,
            owner,
            updater,
        }
    }
}

pub(crate) struct RowRender<'scope, T> {
    inner: Rc<dyn Fn(RowRenderArgs<'scope, T>) + 'scope>,
}

impl<'scope, T> Clone for RowRender<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'scope, T> RowRender<'scope, T> {
    pub(crate) fn new<F>(render: F) -> Self
    where
        F: Fn(RowRenderArgs<'scope, T>) + 'scope,
    {
        Self {
            inner: Rc::new(render),
        }
    }

    pub(crate) fn call(&self, args: RowRenderArgs<'scope, T>) {
        (self.inner)(args);
    }
}

#[derive(Clone)]
pub(crate) struct DomRange {
    pub(crate) start: Node,
    pub(crate) end: Node,
}

struct RangeGuard(Option<DomRange>);

impl RangeGuard {
    fn new(range: DomRange) -> Self {
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

impl DomRange {
    pub(crate) fn append(parent: &Node, label: &str) -> Result<Self, SilexError> {
        let document = crate::document();
        let start: Node = document.create_comment(&format!("{label}-start")).into();
        let end: Node = document.create_comment(&format!("{label}-end")).into();
        parent.append_child(&start).map_err(SilexError::from)?;
        if let Err(error) = parent.append_child(&end).map_err(SilexError::from) {
            let _ = parent.remove_child(&start);
            return Err(error);
        }
        Ok(Self { start, end })
    }

    pub(crate) fn before(reference: &Node, label: &str) -> Result<Self, SilexError> {
        let Some(parent) = reference.parent_node() else {
            return Err(SilexError::Dom(
                "cannot create a row range without a parent".to_string(),
            ));
        };
        let document = crate::document();
        let start: Node = document.create_comment(&format!("{label}-start")).into();
        let end: Node = document.create_comment(&format!("{label}-end")).into();
        parent
            .insert_before(&start, Some(reference))
            .map_err(SilexError::from)?;
        if let Err(error) = parent
            .insert_before(&end, Some(reference))
            .map_err(SilexError::from)
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

    pub(crate) fn move_before(&self, reference: &Node) {
        let Some(parent) = reference.parent_node() else {
            return;
        };
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
            return;
        }
        for node in nodes {
            let _ = parent.insert_before(&node, Some(reference));
        }
    }
}

pub(crate) struct RowController<'scope, T> {
    range: DomRange,
    row_scope: Rc<OwnedScope<'scope>>,
    render_scope: Option<Rc<OwnedScope<'scope>>>,
    render_nodes: Rc<RefCell<Vec<Node>>>,
    render: RowRender<'scope, T>,
    render_inputs: RuntimeInputs,
    attrs: Vec<PendingAttribute<'scope>>,
    reporter: ErrorReporter<'scope>,
    updater: RowUpdater<'scope, T>,
    stateful: bool,
    active: Cell<bool>,
    generation: u64,
    marker: PhantomData<fn(T)>,
}

impl<'scope, T: Clone + 'scope> RowController<'scope, T> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        owner: &dyn ViewOwner<'scope>,
        range: DomRange,
        render: RowRender<'scope, T>,
        render_inputs: RuntimeInputs,
        attrs: Vec<PendingAttribute<'scope>>,
        item: T,
        index: usize,
        stateful: bool,
    ) -> Option<Self> {
        let mut range_guard = RangeGuard::new(range.clone());
        let updater = RowUpdater::new();
        let reporter = owner.token().error_reporter();
        let mut controller = Self {
            range,
            row_scope: Rc::new(owner.owned_scope()),
            render_scope: None,
            render_nodes: Rc::new(RefCell::new(Vec::new())),
            render,
            render_inputs,
            attrs,
            reporter,
            updater,
            stateful,
            active: Cell::new(true),
            generation: 0,
            marker: PhantomData,
        };
        if !controller.mount_render(item, index) {
            controller.dispose();
            return None;
        }
        if stateful && !controller.updater.is_active() {
            controller.dispose();
            return None;
        }
        range_guard.disarm();
        Some(controller)
    }

    pub(crate) fn update(&mut self, item: T, index: usize) -> bool {
        if !self.active.get() {
            return false;
        }
        if self.stateful {
            return self.updater.update(item, index);
        }
        self.mount_render(item, index)
    }

    fn mount_render(&mut self, item: T, index: usize) -> bool {
        let previous_scope = self.render_scope.take();
        let previous_nodes = self.render_nodes.borrow().clone();
        let render_scope = Rc::new(self.row_scope.child());
        let render_owner = OwnedViewOwner::new(render_scope.clone(), self.reporter.clone());
        let range = self.range.clone();
        let render = self.render.clone();
        let attrs = self.attrs.clone();
        let updater = self.updater.clone();
        let rendered_nodes = Rc::new(RefCell::new(previous_nodes));
        let render_failed = Rc::new(Cell::new(false));
        let render_failed_for_effect = render_failed.clone();
        let rendered_nodes_for_effect = rendered_nodes.clone();
        let document = crate::document();
        let result = catch_unwind(AssertUnwindSafe(|| {
            render_scope.effect_from(self.render_inputs.clone(), move || {
                let old_nodes = rendered_nodes_for_effect.borrow().clone();
                let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), SilexError> {
                    let fragment = document.create_document_fragment();
                    let fragment_node: Node = fragment.into();
                    let token = render_owner.token();
                    render.call(RowRenderArgs::new(
                        item.clone(),
                        index,
                        fragment_node.clone(),
                        attrs.clone(),
                        token,
                        updater.clone(),
                    ));
                    let new_nodes = child_nodes(&fragment_node);
                    let Some(parent) = range.end.parent_node() else {
                        return Err(SilexError::Dom(
                            "cannot commit row render without a parent".to_string(),
                        ));
                    };
                    parent
                        .insert_before(&fragment_node, Some(&range.end))
                        .map_err(SilexError::from)?;
                    for node in old_nodes {
                        if node.parent_node().is_some() {
                            let _ = parent.remove_child(&node);
                        }
                    }
                    *rendered_nodes_for_effect.borrow_mut() = new_nodes;
                    Ok(())
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        render_failed_for_effect.set(true);
                        render_owner.report_error(error);
                    }
                    Err(panic) => {
                        render_failed_for_effect.set(true);
                        let message = panic_message(&panic, "Row render");
                        render_owner.report_error(SilexError::Javascript(message));
                    }
                }
            });
        }));
        if let Err(panic) = result {
            render_failed.set(true);
            let message = panic_message(&panic, "Row effect");
            self.reporter.report(SilexError::Javascript(message));
        }

        if render_failed.get() {
            let _ = catch_unwind(AssertUnwindSafe(|| render_scope.dispose()));
            self.render_scope = previous_scope;
            return false;
        }

        if let Some(scope) = previous_scope
            && let Err(panic) = catch_unwind(AssertUnwindSafe(|| scope.dispose()))
        {
            let message = panic_message(&panic, "Previous row render cleanup");
            self.reporter.report(SilexError::Javascript(message));
        }
        self.render_scope = Some(render_scope);
        self.render_nodes = rendered_nodes;
        true
    }
}

impl<'scope, T> RowController<'scope, T> {
    pub(crate) fn move_before(&self, reference: &Node) {
        if self.active.get() {
            self.range.move_before(reference);
        }
    }

    pub(crate) fn dispose(&mut self) {
        if !self.active.replace(false) {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.updater.invalidate();
        let panic = self.dispose_owners();
        self.range.remove();
        if let Some(panic) = panic {
            resume_unwind(panic);
        }
    }

    fn dispose_owners(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        let mut first_panic = None;
        if let Some(scope) = self.render_scope.take()
            && let Err(panic) = catch_unwind(AssertUnwindSafe(|| scope.dispose()))
        {
            first_panic = Some(panic);
        }
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| self.row_scope.dispose()))
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
        first_panic
    }
}

fn child_nodes(parent: &Node) -> Vec<Node> {
    let children = parent.child_nodes();
    (0..children.length())
        .filter_map(|index| children.item(index))
        .collect()
}

impl<T> Drop for RowController<'_, T> {
    fn drop(&mut self) {
        self.dispose();
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
