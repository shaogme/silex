use crate::attribute::PendingAttribute;
use crate::view::{OwnedViewOwner, ViewOwner, ViewOwnerToken};
use silex_core::{OwnedScope, SilexError};
use std::{
    cell::Cell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use web_sys::Node;

pub(crate) struct RowRenderArgs<'scope, 'run, T> {
    pub(crate) item: T,
    pub(crate) index: usize,
    pub(crate) parent: Node,
    pub(crate) attrs: Vec<PendingAttribute<'scope, 'run>>,
    pub(crate) owner: ViewOwnerToken<'scope, 'run>,
}

impl<'scope, 'run, T> RowRenderArgs<'scope, 'run, T> {
    pub(crate) fn new(
        item: T,
        index: usize,
        parent: Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
        owner: ViewOwnerToken<'scope, 'run>,
    ) -> Self {
        Self {
            item,
            index,
            parent,
            attrs,
            owner,
        }
    }
}

pub(crate) struct RowRender<'scope, 'run, T> {
    inner: Rc<dyn Fn(RowRenderArgs<'scope, 'run, T>) + 'scope>,
}

impl<'scope, 'run, T> Clone for RowRender<'scope, 'run, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'scope, 'run, T> RowRender<'scope, 'run, T> {
    pub(crate) fn new<F>(render: F) -> Self
    where
        F: Fn(RowRenderArgs<'scope, 'run, T>) + 'scope,
    {
        Self {
            inner: Rc::new(render),
        }
    }

    pub(crate) fn call(&self, args: RowRenderArgs<'scope, 'run, T>) {
        (self.inner)(args);
    }
}

#[derive(Clone)]
pub(crate) struct DomRange {
    pub(crate) start: Node,
    pub(crate) end: Node,
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

pub(crate) struct RowController<'scope, 'run, T> {
    range: DomRange,
    row_scope: Rc<OwnedScope<'scope, 'run>>,
    render_scope: Option<Rc<OwnedScope<'scope, 'run>>>,
    render: RowRender<'scope, 'run, T>,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
    active: Cell<bool>,
    generation: u64,
    marker: PhantomData<fn(T)>,
}

impl<'scope, 'run, T: Clone + 'scope> RowController<'scope, 'run, T>
where
    'run: 'scope,
{
    pub(crate) fn new(
        owner: &dyn ViewOwner<'scope, 'run>,
        range: DomRange,
        render: RowRender<'scope, 'run, T>,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
        item: T,
        index: usize,
    ) -> Self {
        let mut controller = Self {
            range,
            row_scope: Rc::new(owner.owned_scope()),
            render_scope: None,
            render,
            attrs,
            active: Cell::new(true),
            generation: 0,
            marker: PhantomData,
        };
        controller.mount_render(item, index);
        controller
    }

    pub(crate) fn update(&mut self, item: T, index: usize) -> bool {
        if !self.active.get() {
            return false;
        }
        self.mount_render(item, index);
        true
    }

    fn mount_render(&mut self, item: T, index: usize) {
        if let Some(scope) = self.render_scope.take() {
            let result = catch_unwind(AssertUnwindSafe(|| scope.dispose()));
            if let Err(panic) = result {
                resume_unwind(panic);
            }
        }
        self.range.clear();

        let render_scope = Rc::new(self.row_scope.child());
        let render_owner = OwnedViewOwner::new(render_scope.clone());
        let range = self.range.clone();
        let render = self.render.clone();
        let attrs = self.attrs.clone();
        let document = crate::document();
        let result = catch_unwind(AssertUnwindSafe(|| {
            render_scope.effect(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    range.clear();
                    let fragment = document.create_document_fragment();
                    let fragment_node: Node = fragment.into();
                    let token = render_owner.token();
                    render.call(RowRenderArgs::new(
                        item.clone(),
                        index,
                        fragment_node.clone(),
                        attrs.clone(),
                        token,
                    ));
                    if let Some(parent) = range.end.parent_node() {
                        let _ = parent.insert_before(&fragment_node, Some(&range.end));
                    }
                }));
                if let Err(panic) = result {
                    let message = panic_message(&panic, "Row render");
                    silex_core::error::handle_error(SilexError::Javascript(message));
                }
            });
        }));
        if let Err(panic) = result {
            let message = panic_message(&panic, "Row effect");
            silex_core::error::handle_error(SilexError::Javascript(message));
        }
        self.render_scope = Some(render_scope);
    }
}

impl<'scope, 'run, T> RowController<'scope, 'run, T> {
    pub(crate) fn move_before(&self, reference: &Node) {
        if self.active.get() {
            self.range.move_before(reference);
        }
    }

    pub(crate) fn dispose_keep_range(&mut self) {
        if !self.active.replace(false) {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let panic = self.dispose_owners();
        self.range.clear();
        if let Some(panic) = panic {
            resume_unwind(panic);
        }
    }

    pub(crate) fn dispose(&mut self) {
        if !self.active.replace(false) {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
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

impl<T> Drop for RowController<'_, '_, T> {
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
