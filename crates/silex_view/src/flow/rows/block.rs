use crate::flow::context::BranchRenderContext;
use crate::flow::rows::{
    cleanup::{close_scope, panic_message},
    range::RangeHandle,
    renderer::{RowRenderContext, RowRenderer},
    updater::RowUpdater,
};
use crate::kernel::{MountContext, MountTarget, MountTransaction};
use crate::lifecycle::{MountErrorHandler, MountOwner, MountOwnerToken};
use silex_core::{
    CloseError, ClosePhase, CloseSource, CloseTransaction, EffectPhase, OwnerChild, ReactiveError,
    SilexError, SilexErrorKind, SilexResult,
};
use silex_dom::{
    diagnostics::DomError,
    model::{DomNode, NodeKind},
    runtime::InsertRequest,
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

pub(crate) enum RowState {
    Detached,
    Mounted,
    Disposed,
}

struct MountedContent<'scope> {
    nodes: Vec<DomNode>,
    owner: MountOwnerToken<'scope>,
}

pub(crate) struct RowBlock<'scope, T> {
    range: RangeHandle,
    row_scope: MountOwnerToken<'scope>,
    content_owner: Option<MountOwnerToken<'scope>>,
    runtime_child: Option<OwnerChild<'scope>>,
    render_scope: Option<MountOwnerToken<'scope>>,
    content: Rc<RefCell<Option<MountedContent<'scope>>>>,
    render: RowRenderer<'scope, T>,
    error_handler: MountErrorHandler<'scope>,
    context: MountContext<'scope>,
    updater: RowUpdater<'scope, T>,
    stateful: bool,
    state: RowState,
    item: T,
    index: usize,
}

pub(crate) struct RowBlockConfig<'scope, T> {
    pub(crate) range: RangeHandle,
    pub(crate) render: RowRenderer<'scope, T>,
    pub(crate) item: T,
    pub(crate) index: usize,
    pub(crate) stateful: bool,
    pub(crate) branch_runtime: bool,
    pub(crate) error_handler: MountErrorHandler<'scope>,
    pub(crate) context: MountContext<'scope>,
}

impl<'scope, T: Clone + 'scope> RowBlock<'scope, T> {
    pub(crate) fn empty(
        owner: &dyn MountOwner<'scope>,
        config: RowBlockConfig<'scope, T>,
    ) -> SilexResult<Self> {
        let RowBlockConfig {
            range,
            render,
            item,
            index,
            stateful,
            branch_runtime,
            error_handler,
            context,
        } = config;
        let row_scope = owner.child();
        let (runtime_child, content_owner) = if branch_runtime {
            let (child, owner) = row_scope.branch_child()?;
            (Some(child), Some(owner))
        } else {
            (None, None)
        };
        let state = range.initial_state()?;
        Ok(Self {
            range,
            row_scope,
            content_owner,
            runtime_child,
            render_scope: None,
            content: Rc::new(RefCell::new(None)),
            render,
            error_handler,
            context,
            updater: RowUpdater::new(),
            stateful,
            state,
            item,
            index,
        })
    }

    pub(crate) fn new(
        owner: &dyn MountOwner<'scope>,
        config: RowBlockConfig<'scope, T>,
    ) -> SilexResult<Self> {
        let range = config.range.clone();
        let mut guard = range.guard();
        let mut row = Self::empty(owner, config)?;
        let item = row.item.clone();
        let index = row.index;
        if let Err(error) = row.mount_render(item, index) {
            if let Err(close_error) = row.dispose() {
                row.row_scope.report_close_error(close_error);
            }
            return Err(error);
        }
        if row.stateful && !row.updater.is_active() {
            if let Err(close_error) = row.dispose() {
                row.row_scope.report_close_error(close_error);
            }
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "stateful row updater was not bound during initial render".into(),
            )));
        }
        row.ensure_invariant()?;
        RangeHandle::disarm_guard(&mut guard);
        Ok(row)
    }

    pub(crate) fn update(&mut self, item: T, index: usize) -> SilexResult<()> {
        self.ensure_usable()?;
        self.ensure_invariant()?;
        let next = item.clone();
        let result = if self.stateful {
            self.updater.update(item, index)
        } else {
            self.mount_render(item, index)
        };
        if result.is_ok() {
            self.item = next;
            self.index = index;
            self.ensure_invariant()?;
        }
        result
    }

    pub(crate) fn snapshot(&self) -> (T, usize) {
        (self.item.clone(), self.index)
    }

    fn mount_render(&mut self, item: T, index: usize) -> SilexResult<()> {
        self.ensure_usable()?;
        let previous_render_scope = self.render_scope.take();
        let render_scope = self.row_scope.child();
        let content = self.content.clone();
        let context = self.context.clone();
        let range = self.range.clone();
        let render = self.render.clone();
        let content_owner = self.content_owner.clone();
        let row_scope = self.row_scope.clone();
        let error_handler = self.error_handler;
        let updater = self.updater.clone();
        let render_scope_for_cleanup = render_scope.clone();
        let result = render_scope.effect(
            EffectPhase::Normal,
            Box::new(move || {
                let candidate_scope = content_owner
                    .as_ref()
                    .map(MountOwnerToken::child)
                    .unwrap_or_else(|| row_scope.child());
                let fragment = match context.dom().create_fragment() {
                    Ok(fragment) => fragment,
                    Err(error) => {
                        let _ = close_scope(candidate_scope.clone(), &candidate_scope);
                        return Err(error.into());
                    }
                };
                let render_transaction = MountTransaction::new();
                let render_context = context.with_parts(
                    MountTarget::append(context.dom().clone(), fragment.clone()),
                    context.ancestry().clone(),
                    candidate_scope.clone(),
                    render_transaction.clone(),
                );
                let rendered = catch_unwind(AssertUnwindSafe(|| {
                    render.call(RowRenderContext {
                        item: item.clone(),
                        index,
                        context: render_context,
                        branch_context: content_owner.as_ref().map(|_| {
                            BranchRenderContext::new(
                                candidate_scope.clone(),
                                candidate_scope.runtime_access(),
                                error_handler,
                            )
                        }),
                        updater: updater.clone(),
                    })
                }));
                let rendered = match rendered {
                    Ok(result) => result,
                    Err(panic) => Err(SilexError::fatal(SilexErrorKind::Javascript(
                        panic_message("Row render", panic),
                    ))),
                };
                if let Err(error) = rendered {
                    let _ = candidate_scope.close();
                    return Err(error);
                }
                if let Err(error) = render_transaction.commit() {
                    let _ = candidate_scope.close();
                    return Err(error);
                }
                let new_nodes = match context.dom().children(&fragment) {
                    Ok(nodes) => nodes,
                    Err(error) => {
                        let _ = close_scope(candidate_scope.clone(), &candidate_scope);
                        return Err(error.into());
                    }
                };
                let parent = match context.dom().parent(&range.end) {
                    Ok(Some(parent)) => parent,
                    Ok(None) => {
                        let _ = close_scope(candidate_scope.clone(), &candidate_scope);
                        return Err(SilexError::from(DomError::NoParent));
                    }
                    Err(error) => {
                        let _ = close_scope(candidate_scope.clone(), &candidate_scope);
                        return Err(error.into());
                    }
                };
                if let Err(error) = context
                    .dom()
                    .insert_before(InsertRequest::before(&parent, &fragment, &range.end))
                {
                    let _ = candidate_scope.close();
                    return Err(error.into());
                }
                let previous = content.borrow_mut().replace(MountedContent {
                    nodes: new_nodes,
                    owner: candidate_scope,
                });
                if let Some(previous) = previous
                    && let Err(error) = close_scope(previous.owner, &render_scope_for_cleanup)
                {
                    return Err(SilexError::fatal(SilexErrorKind::Close(error)));
                }
                Ok(())
            }),
            error_handler,
        );
        if let Err(error) = result {
            self.render_scope = previous_render_scope;
            let _ = render_scope.close();
            return Err(error);
        }
        if let Some(previous_scope) = previous_render_scope
            && let Err(error) = close_scope(previous_scope, &self.row_scope)
        {
            return Err(SilexError::fatal(SilexErrorKind::Close(error)));
        }
        self.render_scope = Some(render_scope);
        let parent = self.range.parent()?;
        self.state = if parent.kind() == NodeKind::Fragment {
            RowState::Detached
        } else {
            RowState::Mounted
        };
        self.ensure_invariant()
    }
}

impl<'scope, T> RowBlock<'scope, T> {
    pub(crate) fn move_before(
        &mut self,
        target_parent: &DomNode,
        reference: &DomNode,
    ) -> SilexResult<()> {
        self.ensure_usable()?;
        self.ensure_invariant()?;
        self.range
            .dom_range()?
            .move_before(target_parent, reference)
            .map_err(SilexError::from)?;
        self.state = RowState::Mounted;
        self.ensure_invariant()
    }

    pub(crate) fn dispose(&mut self) -> Result<(), CloseError> {
        if matches!(self.state, RowState::Disposed) {
            return Ok(());
        }
        self.state = RowState::Disposed;
        self.updater.invalidate();
        let mut transaction = CloseTransaction::new();
        if let Some(content) = self.content.borrow_mut().take()
            && let Err(error) = close_scope(content.owner, &self.row_scope)
        {
            transaction.push_error(ClosePhase::Child, CloseSource::Child, error);
        }
        if let Some(render_scope) = self.render_scope.take()
            && let Err(error) = close_scope(render_scope, &self.row_scope)
        {
            transaction.push_error(ClosePhase::Effect, CloseSource::Effect, error);
        }
        if let Err(error) = close_scope(self.row_scope.clone(), &self.row_scope) {
            transaction.push_error(ClosePhase::Child, CloseSource::Owner, error);
        }
        if let Some(child) = &self.runtime_child
            && let Err(error) = child.close()
        {
            transaction.push_error(ClosePhase::Runtime, CloseSource::Owner, error);
        }
        if let Err(error) = self.range.remove() {
            transaction.push_error(
                ClosePhase::Boundary,
                CloseSource::Dispose,
                CloseError::from_panic(Box::new(error.to_string())),
            );
        }
        transaction.finish().map_or(Ok(()), Err)
    }

    fn ensure_usable(&self) -> SilexResult<()> {
        if matches!(self.state, RowState::Disposed) {
            Err(SilexError::fatal(ReactiveError::NoSuchNode))
        } else {
            Ok(())
        }
    }

    fn ensure_invariant(&self) -> SilexResult<()> {
        if matches!(self.state, RowState::Disposed) {
            return Ok(());
        }
        let nodes = self.range.nodes()?;
        if nodes.len() < 2
            || nodes.first() != Some(&self.range.start)
            || nodes.last() != Some(&self.range.end)
        {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "row block range anchors are invalid".into(),
            )));
        }
        let actual_content = &nodes[1..nodes.len() - 1];
        let content = self.content.borrow();
        let recorded_content = content
            .as_ref()
            .map(|content| content.nodes.as_slice())
            .unwrap_or(&[]);
        if actual_content != recorded_content {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "row block content does not match its range".into(),
            )));
        }
        if let Some(content) = content.as_ref()
            && !content.owner.is_active()?
        {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "row block content owner is inactive".into(),
            )));
        }
        if self.stateful && !self.updater.is_active() {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "mounted stateful row updater is inactive".into(),
            )));
        }
        Ok(())
    }
}

impl<T> Drop for RowBlock<'_, T> {
    fn drop(&mut self) {
        if let Err(error) = self.dispose() {
            self.row_scope.report_close_error(error);
        }
    }
}
