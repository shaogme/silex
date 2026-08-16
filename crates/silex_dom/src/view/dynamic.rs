use super::any::AnyView;
use super::contract::{ApplyAttributes, MountInstance, View};
use super::owner::{MountErrorHandler, MountOwner, MountOwnerToken};
use super::row::{NodeRange, RowInstance, RowInstanceConfig, RowRenderContext, RowRenderer};
use crate::attribute::PendingAttribute;
use silex_core::{
    CloseError, ErrorHandlerInput, OwnerAccess, SilexError, SilexErrorKind, SilexResult,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use web_sys::Node;

pub struct DynamicRenderArgs<'scope> {
    pub(crate) parent: Node,
    pub(crate) attrs: Vec<PendingAttribute<'scope>>,
    pub(crate) owner: MountOwnerToken<'scope>,
    pub(crate) error_handler: MountErrorHandler<'scope>,
}

impl<'scope> DynamicRenderArgs<'scope> {
    pub fn new(
        parent: Node,
        attrs: Vec<PendingAttribute<'scope>>,
        owner: MountOwnerToken<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> Self {
        Self {
            parent,
            attrs,
            owner,
            error_handler,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        Node,
        Vec<PendingAttribute<'scope>>,
        MountOwnerToken<'scope>,
        MountErrorHandler<'scope>,
    ) {
        (self.parent, self.attrs, self.owner, self.error_handler)
    }
}

pub struct DynamicRenderer<'scope> {
    inner: silex_vtable::thunk::ThunkBox<
        'scope,
        DynamicRenderArgs<'scope>,
        SilexResult<MountInstance<'scope>>,
    >,
}

impl<'scope> DynamicRenderer<'scope> {
    pub fn new<F>(render: F) -> Self
    where
        F: Fn(DynamicRenderArgs<'scope>) -> SilexResult<MountInstance<'scope>> + 'scope,
    {
        Self {
            inner: silex_vtable::thunk::ThunkBox::new(render),
        }
    }

    pub fn call(&self, args: DynamicRenderArgs<'scope>) -> SilexResult<MountInstance<'scope>> {
        self.inner.call(args)
    }
}

impl<'scope, F, V> ApplyAttributes<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
}

impl<'scope, F, V> View<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let factory = self.clone();
        mount_dynamic_view_universal(
            owner,
            parent,
            attrs,
            error_handler,
            DynamicRenderer::new(move |args| {
                let DynamicRenderArgs {
                    parent,
                    attrs,
                    owner: token,
                    error_handler,
                } = args;
                let view = factory();
                view.mount(&token, &parent, attrs, error_handler)
            }),
        )
    }
}

/// Shared dynamic-view mount kernel.
pub fn mount_dynamic_view_universal<'scope>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    renderer: DynamicRenderer<'scope>,
) -> SilexResult<MountInstance<'scope>> {
    let range = NodeRange::append(parent, "dyn")?;
    let render = RowRenderer::new(move |args: RowRenderContext<'scope, ()>| {
        let RowRenderContext {
            parent,
            attrs,
            owner: token,
            error_handler,
            ..
        } = args;
        renderer
            .call(DynamicRenderArgs::new(parent, attrs, token, error_handler))
            .map(|_| ())
    });
    let token = owner.token();
    let range_instance = range.clone();
    let row = match RowInstance::new(
        &token,
        RowInstanceConfig {
            range,
            render,
            attrs,
            item: (),
            index: 0,
            stateful: false,
            branch_runtime: false,
            error_handler,
        },
    ) {
        Ok(row) => row,
        Err(error) => {
            range_instance.remove();
            return Err(error);
        }
    };
    let row_state = owner.token().owner_state(Some(row))?;
    let cleanup_state = row_state.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            if let Some(mut row) = cleanup_state.take_for_cleanup().flatten() {
                row.dispose()
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))?;
            }
            Ok(())
        }),
        error_handler,
    ) {
        if let Some(mut row) = row_state.take_for_cleanup().flatten() {
            if let Err(close_error) = row.dispose() {
                token.report_close_error(close_error);
            }
        }
        range_instance.remove();
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![
        range_instance.start,
        range_instance.end,
    ]))
}

/// The identity and render snapshot produced by one stable branch evaluation.
///
/// Stable branches compare only `key`; the snapshot is delivered to the branch
/// renderer when a new row is mounted.
#[derive(Clone)]
pub struct BranchEvaluation<K, S> {
    key: K,
    snapshot: S,
}

impl<K, S> BranchEvaluation<K, S> {
    pub fn new(key: K, snapshot: S) -> Self {
        Self { key, snapshot }
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_parts(self) -> (K, S) {
        (self.key, self.snapshot)
    }
}

impl<K: PartialEq, S> PartialEq for BranchEvaluation<K, S> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<K: Eq, S> Eq for BranchEvaluation<K, S> {}

/// Capabilities supplied to a stable branch renderer.
///
/// The content token owns the branch DOM state, while [`Self::owner`] points
/// at the persistent runtime child created for this branch. Structural route
/// effects and page runtime nodes therefore have distinct owner identities.
#[derive(Clone)]
pub struct BranchRenderContext<'scope> {
    content_owner: MountOwnerToken<'scope>,
    owner: OwnerAccess<'scope>,
    error_handler: MountErrorHandler<'scope>,
}

impl<'scope> BranchRenderContext<'scope> {
    pub(crate) fn new(
        content_owner: MountOwnerToken<'scope>,
        owner: OwnerAccess<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> Self {
        Self {
            content_owner,
            owner,
            error_handler,
        }
    }

    /// Return the persistent runtime owner for this branch.
    pub fn owner(&self) -> OwnerAccess<'scope> {
        self.owner
    }

    /// Return the DOM content owner for this render.
    pub fn content_owner(&self) -> MountOwnerToken<'scope> {
        self.content_owner.clone()
    }

    /// Return the branch-safe error handler view.
    pub fn error_handler(&self) -> MountErrorHandler<'scope> {
        self.error_handler
    }
}

/// Mount a stable branch whose evaluation can report a runtime error.
pub fn mount_branch_stable_cached<'scope, K, S, KeyFn, BranchFn, H>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: H,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) -> SilexResult<MountInstance<'scope>>
where
    K: PartialEq + Clone + 'scope,
    S: Clone + 'scope,
    KeyFn: Fn() -> SilexResult<BranchEvaluation<K, S>> + Clone + 'scope,
    BranchFn: Fn(BranchEvaluation<K, S>, BranchRenderContext<'scope>) -> AnyView<'scope> + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    let error_handler = error_handler.handler_ref();
    let render = RowRenderer::new(
        move |args: RowRenderContext<'scope, BranchEvaluation<K, S>>| {
            let RowRenderContext {
                item: key,
                parent,
                attrs,
                owner: token,
                error_handler: _,
                branch_context,
                ..
            } = args;
            let branch_context = branch_context.ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Framework(
                    "stable branch render context is missing".to_string(),
                ))
            })?;
            branch_fn(key, branch_context.clone())
                .mount(&token, &parent, attrs, branch_context.error_handler())
                .map(|_| ())
        },
    );
    mount_keyed_dynamic_view(KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        error_handler,
        key_fn,
        render,
        update_same_key: false,
        branch_runtime: true,
    })
}

struct BranchState<'scope, K> {
    range: NodeRange,
    row: Option<RowInstance<'scope, K>>,
    key: Option<K>,
    render: RowRenderer<'scope, K>,
    attrs: Vec<PendingAttribute<'scope>>,
}

struct KeyedDynamicMountArgs<'owner, 'scope, K, KeyFn> {
    owner: &'owner dyn MountOwner<'scope>,
    parent: &'owner Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    key_fn: KeyFn,
    render: RowRenderer<'scope, K>,
    update_same_key: bool,
    branch_runtime: bool,
}

fn mount_keyed_dynamic_view<'scope, K, KeyFn>(
    args: KeyedDynamicMountArgs<'_, 'scope, K, KeyFn>,
) -> SilexResult<MountInstance<'scope>>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> SilexResult<K> + Clone + 'scope,
{
    let KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        error_handler,
        key_fn,
        render,
        update_same_key,
        branch_runtime,
    } = args;
    let local_owner = owner.child();
    let range = NodeRange::append(parent, "branch")?;
    let state = local_owner.token().owner_state(BranchState {
        range,
        row: None,
        key: None,
        render,
        attrs,
    })?;
    let cleanup_state = state.clone();
    let cleanup_range = state.with(|state| state.range.clone())?;
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let Some(mut state) = cleanup_state.take_for_cleanup() else {
                return Ok(());
            };
            state.key = None;
            let row = state.row.take();
            let range = state.range.clone();
            let close_error = row.and_then(|mut row| {
                match catch_unwind(AssertUnwindSafe(move || row.dispose())) {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(panic) => Some(CloseError::from_panic(panic)),
                }
            });
            range.remove();
            close_error.map_or(Ok(()), |error| {
                Err(SilexError::fatal(SilexErrorKind::Close(error)))
            })
        }),
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        cleanup_range.remove();
        return Err(error);
    }

    let token = local_owner.token();
    let effect_state = state.clone();
    if let Err(error) = local_owner.effect(
        Box::new(move || -> SilexResult<()> {
            let mut state = effect_state.take()?;
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                let key = key_fn()?;
                let same_key = state.key.as_ref().is_some_and(|current| current == &key);
                if same_key {
                    if update_same_key {
                        state
                            .row
                            .as_mut()
                            .ok_or_else(|| {
                                SilexError::fatal(SilexErrorKind::Framework(
                                    "dynamic row is missing for current key".to_string(),
                                ))
                            })?
                            .update(key, 0)?;
                    } else if state.row.is_none() {
                        return Err(SilexError::fatal(SilexErrorKind::Framework(
                            "dynamic row is missing for current key".to_string(),
                        )));
                    }
                    return Ok(());
                }

                let (outer_range, render, attrs, old_row, old_key) = {
                    (
                        state.range.clone(),
                        state.render.clone(),
                        state.attrs.clone(),
                        state.row.take(),
                        state.key.take(),
                    )
                };
                let row_range = match NodeRange::before(&outer_range.end, "branch-row") {
                    Ok(row_range) => row_range,
                    Err(error) => {
                        state.row = old_row;
                        state.key = old_key;
                        return Err(error);
                    }
                };
                let row = match RowInstance::new(
                    &token,
                    RowInstanceConfig {
                        range: row_range,
                        render,
                        attrs,
                        item: key.clone(),
                        index: 0,
                        stateful: false,
                        branch_runtime,
                        error_handler,
                    },
                ) {
                    Ok(row) => row,
                    Err(error) => {
                        state.row = old_row;
                        state.key = old_key;
                        return Err(error);
                    }
                };

                let old_close_error = old_row.and_then(|mut row| {
                    match catch_unwind(AssertUnwindSafe(move || row.dispose())) {
                        Ok(Ok(())) => None,
                        Ok(Err(error)) => Some(error),
                        Err(panic) => Some(CloseError::from_panic(panic)),
                    }
                });
                state.key = Some(key);
                state.row = Some(row);
                old_close_error.map_or(Ok(()), |error| {
                    Err(SilexError::fatal(SilexErrorKind::Close(error)))
                })
            }));
            effect_state.replace(state)?;
            match result {
                Ok(result) => result,
                Err(panic) => {
                    let message = if let Some(value) = panic.downcast_ref::<&str>() {
                        format!("Panic in Dynamic Branch: {value}")
                    } else if let Some(value) = panic.downcast_ref::<String>() {
                        format!("Panic in Dynamic Branch: {value}")
                    } else {
                        "Panic in Dynamic Branch: unknown panic".to_string()
                    };
                    Err(SilexError::fatal(SilexErrorKind::Javascript(message)))
                }
            }
        }),
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }
    let owner_for_cleanup = local_owner.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            owner_for_cleanup
                .close()
                .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
        }),
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![
        cleanup_range.start,
        cleanup_range.end,
    ]))
}
