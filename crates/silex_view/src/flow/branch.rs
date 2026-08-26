use crate::flow::context::BranchRenderContext;
use crate::flow::rows::{RangeHandle, RowBlock, RowBlockConfig, RowRenderContext, RowRenderer};
use crate::kernel::elements::AnyView;
use crate::kernel::{MountContext, MountInstance, MountTarget, View};
use crate::lifecycle::MountErrorHandler;
use silex_core::{EffectPhase, SilexError, SilexErrorKind, SilexResult};
use std::rc::Rc;

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
pub struct StableBranch<'scope, K, S> {
    key_fn: Rc<dyn Fn() -> SilexResult<BranchEvaluation<K, S>> + 'scope>,
    branch_fn:
        Rc<dyn Fn(BranchEvaluation<K, S>, BranchRenderContext<'scope>) -> AnyView<'scope> + 'scope>,
}

impl<'scope, K, S> StableBranch<'scope, K, S> {
    pub fn new<KeyFn, BranchFn>(key_fn: KeyFn, branch_fn: BranchFn) -> Self
    where
        KeyFn: Fn() -> SilexResult<BranchEvaluation<K, S>> + 'scope,
        BranchFn:
            Fn(BranchEvaluation<K, S>, BranchRenderContext<'scope>) -> AnyView<'scope> + 'scope,
    {
        Self {
            key_fn: Rc::new(key_fn),
            branch_fn: Rc::new(branch_fn),
        }
    }
}

impl<'scope, K, S> View<'scope> for StableBranch<'scope, K, S>
where
    K: PartialEq + Clone + 'scope,
    S: Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let branch_fn = self.branch_fn.clone();
        let render = RowRenderer::new(
            move |args: RowRenderContext<'scope, BranchEvaluation<K, S>>| {
                let branch_context = args.branch_context.ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Framework(
                        "stable branch render context is missing".into(),
                    ))
                })?;
                let view = branch_fn(args.item, branch_context);
                args.context.mount(&view).map(|_| ())
            },
        );
        let key_fn = self.key_fn.clone();
        mount_keyed_dynamic(KeyedDynamicConfig {
            context: context.clone(),
            error_handler: context.error_handler(),
            key_fn: move || key_fn(),
            render,
            update_same_key: false,
            branch_runtime: true,
        })
    }
}

struct BranchState<'scope, K> {
    range: RangeHandle,
    row: Option<RowBlock<'scope, K>>,
    key: Option<K>,
    render: RowRenderer<'scope, K>,
}

struct KeyedDynamicConfig<'scope, K, KeyFn> {
    context: MountContext<'scope>,
    error_handler: MountErrorHandler<'scope>,
    key_fn: KeyFn,
    render: RowRenderer<'scope, K>,
    update_same_key: bool,
    branch_runtime: bool,
}

fn mount_keyed_dynamic<'scope, K, KeyFn>(
    config: KeyedDynamicConfig<'scope, K, KeyFn>,
) -> SilexResult<MountInstance<'scope>>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> SilexResult<K> + Clone + 'scope,
{
    let KeyedDynamicConfig {
        context,
        error_handler,
        key_fn,
        render,
        update_same_key,
        branch_runtime,
    } = config;
    let local_owner = context.owner().child();
    let range = RangeHandle::at_target(context.target(), "branch")?;
    let row_context = context.with_target(MountTarget::before(
        context.dom().clone(),
        range.end.clone(),
    ));
    let state = local_owner.owner_state(BranchState {
        range: range.clone(),
        row: None,
        key: None,
        render,
    })?;
    let cleanup_state = state.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let Some(mut state) = cleanup_state.take_for_cleanup() else {
                return Ok(());
            };
            let row_error = state.row.take().and_then(|mut row| row.dispose().err());
            let _ = state.range.remove();
            row_error.map_or(Ok(()), |error| {
                Err(SilexError::fatal(SilexErrorKind::Close(error)))
            })
        }),
        error_handler,
    ) {
        let _ = local_owner.close();
        let _ = range.remove();
        return Err(error);
    }
    let token = local_owner.clone();
    let effect_state = state.clone();
    let effect_context = row_context.clone();
    if let Err(error) = local_owner.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let mut state = effect_state.take()?;
            let key = key_fn()?;
            if state.key.as_ref().is_some_and(|current| current == &key) {
                if update_same_key && let Some(row) = state.row.as_mut() {
                    row.update(key.clone(), 0)?;
                }
                effect_state.replace(state)?;
                return Ok(());
            }
            let old_row = state.row.take();
            let old_key = state.key.take();
            let row_range = match RangeHandle::before(
                &effect_context.dom().clone(),
                &state.range.end,
                "branch-row",
            ) {
                Ok(range) => range,
                Err(error) => {
                    state.row = old_row;
                    state.key = old_key;
                    effect_state.replace(state)?;
                    return Err(error);
                }
            };
            let row = match RowBlock::new(
                &token,
                RowBlockConfig {
                    range: row_range,
                    render: state.render.clone(),
                    item: key.clone(),
                    index: 0,
                    stateful: false,
                    branch_runtime,
                    error_handler,
                    context: effect_context.clone(),
                },
            ) {
                Ok(row) => row,
                Err(error) => {
                    state.row = old_row;
                    state.key = old_key;
                    effect_state.replace(state)?;
                    return Err(error);
                }
            };
            state.key = Some(key);
            state.row = Some(row);
            if let Some(mut old_row) = old_row
                && let Err(error) = old_row.dispose()
            {
                effect_state.replace(state)?;
                return Err(SilexError::fatal(SilexErrorKind::Close(error)));
            }
            effect_state.replace(state).map(|_| ())
        }),
        error_handler,
    ) {
        let _ = local_owner.close();
        let _ = range.remove();
        return Err(error);
    }
    let owner_for_cleanup = local_owner.clone();
    if let Err(error) = context.owner().on_cleanup(
        Box::new(move || {
            owner_for_cleanup
                .close()
                .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
        }),
        error_handler,
    ) {
        let _ = local_owner.close();
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![range.start, range.end]))
}
