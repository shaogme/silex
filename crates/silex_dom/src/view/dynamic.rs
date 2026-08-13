use super::any::AnyView;
use super::contract::{ApplyAttributes, View, ViewFactory};
use super::owner::{MountErrorHandler, MountOwner, MountOwnerToken, OwnedMountOwner};
use super::row::{NodeRange, RowInstance, RowInstanceConfig, RowRenderContext, RowRenderer};
use crate::attribute::PendingAttribute;
use silex_core::{RuntimeInputs, SilexError, SilexErrorKind, SilexResult};
use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
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
}

pub struct DynamicRenderer<'scope> {
    inner: silex_vtable::thunk::ThunkBox<'scope, DynamicRenderArgs<'scope>, SilexResult<()>>,
}

impl<'scope> DynamicRenderer<'scope> {
    pub fn new<F>(render: F) -> Self
    where
        F: Fn(DynamicRenderArgs<'scope>) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: silex_vtable::thunk::ThunkBox::new(render),
        }
    }

    pub fn call(&self, args: DynamicRenderArgs<'scope>) -> SilexResult<()> {
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
    ) -> SilexResult<()> {
        self.clone()
            .mount_owned(owner, parent, attrs, error_handler)
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
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
                let view = self();
                view.create_mount_instance(&token, &parent, attrs, error_handler)
                    .map(|_| ())
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
) -> SilexResult<()> {
    mount_dynamic_view_universal_from(
        owner,
        parent,
        attrs,
        RuntimeInputs::new(),
        error_handler,
        renderer,
    )
}

pub(crate) fn mount_dynamic_view_universal_from<'scope>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: MountErrorHandler<'scope>,
    renderer: DynamicRenderer<'scope>,
) -> SilexResult<()> {
    owner.validate_inputs(&inputs)?;
    let range = NodeRange::append(parent, "dyn")?;
    let render = RowRenderer::new(move |args: RowRenderContext<'scope, ()>| {
        let RowRenderContext {
            parent,
            attrs,
            owner: token,
            error_handler,
            ..
        } = args;
        renderer.call(DynamicRenderArgs::new(parent, attrs, token, error_handler))
    });
    let token = owner.token();
    let row = RowInstance::new(
        &token,
        RowInstanceConfig {
            range,
            render,
            render_inputs: inputs,
            attrs,
            item: (),
            index: 0,
            stateful: false,
            error_handler,
        },
    )?;
    let row_state = owner.token().owner_state(Some(row))?;
    let cleanup_state = row_state.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            if let Some(mut row) = cleanup_state.take_for_cleanup().flatten() {
                row.dispose();
            }
            Ok(())
        }),
        error_handler,
    ) {
        if let Some(mut row) = row_state.take_for_cleanup().flatten() {
            row.dispose();
        }
        return Err(error);
    }
    Ok(())
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

/// Mount a stable branch whose evaluation can report a runtime error.
pub fn mount_branch_stable_cached<'scope, K, S, KeyFn, BranchFn>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: MountErrorHandler<'scope>,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    S: Clone + 'scope,
    KeyFn: Fn() -> SilexResult<BranchEvaluation<K, S>> + Clone + 'scope,
    BranchFn: Fn(BranchEvaluation<K, S>) -> AnyView<'scope> + 'scope,
{
    let render = RowRenderer::new(
        move |args: RowRenderContext<'scope, BranchEvaluation<K, S>>| {
            let RowRenderContext {
                item: key,
                parent,
                attrs,
                owner: token,
                error_handler,
                ..
            } = args;
            branch_fn(key)
                .create_mount_instance(&token, &parent, attrs, error_handler)
                .map(|_| ())
        },
    );
    mount_keyed_dynamic_view(KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key: false,
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
    inputs: RuntimeInputs,
    error_handler: MountErrorHandler<'scope>,
    key_fn: KeyFn,
    render: RowRenderer<'scope, K>,
    update_same_key: bool,
}

fn mount_keyed_dynamic_view<'scope, K, KeyFn>(
    args: KeyedDynamicMountArgs<'_, 'scope, K, KeyFn>,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> SilexResult<K> + Clone + 'scope,
{
    let KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key,
    } = args;
    owner.validate_inputs(&inputs)?;
    let scope = Rc::new(owner.owned_scope()?);
    let local_owner = OwnedMountOwner::new(scope.clone());
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
            let panic = row
                .map(|mut row| catch_unwind(AssertUnwindSafe(move || row.dispose())))
                .and_then(Result::err);
            range.remove();
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
            Ok(())
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        cleanup_range.remove();
        return Err(error);
    }

    let token = local_owner.token();
    let effect_state = state.clone();
    if let Err(error) = local_owner.effect_from(
        inputs,
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
                        render_inputs: RuntimeInputs::new(),
                        attrs,
                        item: key.clone(),
                        index: 0,
                        stateful: false,
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

                let old_panic = old_row
                    .map(|mut row| catch_unwind(AssertUnwindSafe(move || row.dispose())))
                    .and_then(Result::err);
                state.key = Some(key);
                state.row = Some(row);
                if let Some(panic) = old_panic {
                    resume_unwind(panic);
                }
                Ok(())
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
        let _ = scope.dispose();
        return Err(error);
    }
    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            let _ = scope_for_cleanup.dispose();
            Ok(())
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    Ok(())
}
