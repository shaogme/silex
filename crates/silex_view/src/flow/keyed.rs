use crate::flow::list::RowFactory;
use crate::flow::reconcile::{
    dispose_keyed, panic_message, read_values, reconcile_rollback_error, restore_keyed,
    restore_keyed_order,
};
use crate::flow::rows::{RangeHandle, RowBlock, RowBlockConfig, RowRenderContext, RowRenderer};
use crate::kernel::{MountContext, MountInstance, MountTarget};
use crate::lifecycle::MountErrorHandler;
use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use silex_core::{EffectPhase, ErrorHandlerToken, SilexError, SilexErrorKind, SilexResult};
use silex_dom::diagnostics::DomError;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

pub(crate) struct KeyedRow<'scope, T, K> {
    pub(crate) row: RowBlock<'scope, T>,
    pub(crate) marker: PhantomData<K>,
}

pub(crate) struct KeyedState<'scope, T, K> {
    pub(crate) rows: HashMap<K, KeyedRow<'scope, T, K>>,
    pub(crate) order: Vec<K>,
}

pub(crate) struct KeyedListConfig<'scope, IF, T, K> {
    pub(crate) context: MountContext<'scope>,
    pub(crate) source: IF,
    pub(crate) key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub(crate) factory: RowFactory<'scope, T>,
    pub(crate) custom_handler: Option<ErrorHandlerToken<'scope>>,
    pub(crate) parent_handler: MountErrorHandler<'scope>,
}

pub(crate) fn mount_keyed_list<'scope, IF, IS, T, K>(
    config: KeyedListConfig<'scope, IF, T, K>,
) -> SilexResult<MountInstance<'scope>>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    K: Hash + Eq + Clone + 'scope,
{
    let KeyedListConfig {
        context,
        source,
        key_fn,
        factory,
        custom_handler,
        parent_handler,
    } = config;
    let local_owner = context.owner().child();
    let error_handler = custom_handler
        .map(|handler| handler.view())
        .unwrap_or(parent_handler);
    let range = RangeHandle::at_target(context.target(), "for")?;
    let row_context = context.with_target(MountTarget::before(
        context.dom().clone(),
        range.end.clone(),
    ));
    let token = local_owner.clone();
    let stateful = factory.stateful();
    let render_factory = factory.clone();
    let render = RowRenderer::new(move |args: RowRenderContext<'scope, T>| {
        let view = render_factory.render(args.item, args.index, args.updater);
        args.context.mount(&view).map(|_| ())
    });
    let state = local_owner.owner_state(KeyedState {
        rows: HashMap::new(),
        order: Vec::new(),
    })?;
    let cleanup_state = state.clone();
    let cleanup_range = range.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let Some(state) = cleanup_state.take_for_cleanup() else {
                return Ok(());
            };
            let mut rows = state.rows.into_values().collect::<Vec<_>>();
            let error = dispose_keyed(&mut rows);
            let _ = cleanup_range.remove();
            error.map_or(Ok(()), |error| {
                Err(SilexError::fatal(SilexErrorKind::Close(error)))
            })
        }),
        error_handler,
    ) {
        let _ = local_owner.close();
        let _ = range.remove();
        return Err(error);
    }
    let effect_state = state.clone();
    let end = range.end.clone();
    let effect_context = context.clone();
    if let Err(error) = local_owner.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let values = read_values(&source)?;
            let key_result = catch_unwind(AssertUnwindSafe(|| {
                values.iter().map(|value| key_fn(value)).collect::<Vec<_>>()
            }));
            let keys = match key_result {
                Ok(keys) => keys,
                Err(panic) => {
                    return Err(SilexError::fatal(SilexErrorKind::Javascript(
                        panic_message("Keyed list key function", panic),
                    )));
                }
            };
            let mut seen = HashSet::new();
            if keys.iter().any(|key| !seen.insert(key.clone())) {
                return Err(SilexError::fatal(SilexErrorKind::Framework(
                    "duplicate key in keyed list".into(),
                )));
            }
            let mut state = effect_state.take()?;
            let old_state = state.order.clone();
            let parent = effect_context
                .dom()
                .parent(&end)?
                .ok_or_else(|| SilexError::from(DomError::NoParent))?;
            let mut updated = Vec::new();
            let mut pending = HashMap::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                for (index, (key, item)) in
                    keys.iter().cloned().zip(values.iter().cloned()).enumerate()
                {
                    if let Some(row) = state.rows.get_mut(&key) {
                        let snapshot = row.row.snapshot();
                        row.row.update(item, index)?;
                        updated.push((key, snapshot));
                    } else {
                        let row_range =
                            RangeHandle::detached(&effect_context.dom().clone(), "for-row")?;
                        let row = RowBlock::new(
                            &token,
                            RowBlockConfig {
                                range: row_range,
                                render: render.clone(),
                                item,
                                index,
                                stateful,
                                branch_runtime: false,
                                error_handler,
                                context: row_context.clone(),
                            },
                        )?;
                        pending.insert(
                            key.clone(),
                            KeyedRow {
                                row,
                                marker: PhantomData,
                            },
                        );
                    }
                }
                for key in &keys {
                    if let Some(row) = state.rows.get_mut(key) {
                        row.row.move_before(&parent, &end)?;
                    } else if let Some(row) = pending.get_mut(key) {
                        row.row.move_before(&parent, &end)?;
                    }
                }
                Ok(())
            }));
            match result {
                Ok(Ok(())) => {
                    let removed_keys = state
                        .order
                        .iter()
                        .filter(|key| !keys.contains(key))
                        .cloned()
                        .collect::<Vec<_>>();
                    let mut removed = Vec::new();
                    for key in removed_keys {
                        if let Some(row) = state.rows.remove(&key) {
                            removed.push(row);
                        }
                    }
                    for (key, row) in pending {
                        state.rows.insert(key, row);
                    }
                    state.order = keys;
                    let error = dispose_keyed(&mut removed);
                    effect_state.replace(state)?;
                    error.map_or(Ok(()), |error| {
                        Err(SilexError::fatal(SilexErrorKind::Close(error)))
                    })
                }
                Ok(Err(error)) => {
                    let restore_order =
                        restore_keyed_order(&mut state.rows, &old_state, &parent, &end).err();
                    let restore_updates = restore_keyed(&mut state.rows, &updated);
                    state.order = old_state;
                    let mut pending_rows = pending.into_values().collect::<Vec<_>>();
                    let cleanup = dispose_keyed(&mut pending_rows);
                    effect_state.replace(state)?;
                    Err(reconcile_rollback_error(
                        error,
                        restore_order,
                        restore_updates,
                        cleanup,
                    ))
                }
                Err(panic) => {
                    let restore_order =
                        restore_keyed_order(&mut state.rows, &old_state, &parent, &end).err();
                    let restore_updates = restore_keyed(&mut state.rows, &updated);
                    state.order = old_state;
                    let mut pending_rows = pending.into_values().collect::<Vec<_>>();
                    let cleanup = dispose_keyed(&mut pending_rows);
                    effect_state.replace(state)?;
                    let error = SilexError::fatal(SilexErrorKind::Javascript(panic_message(
                        "Keyed list",
                        panic,
                    )));
                    Err(reconcile_rollback_error(
                        error,
                        restore_order,
                        restore_updates,
                        cleanup,
                    ))
                }
            }
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
        parent_handler,
    ) {
        let _ = local_owner.close();
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![range.start, range.end]))
}
