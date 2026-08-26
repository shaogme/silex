use crate::flow::list::RowFactory;
use crate::flow::reconcile::{
    dispose_rows, panic_message, read_values, reconcile_rollback_error, restore_indexed,
};
use crate::flow::rows::{RangeHandle, RowBlock, RowBlockConfig, RowRenderContext, RowRenderer};
use crate::kernel::{MountContext, MountInstance, MountTarget};
use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use silex_core::{EffectPhase, SilexError, SilexErrorKind, SilexResult};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(crate) struct IndexedState<'scope, T> {
    pub(crate) rows: Vec<RowBlock<'scope, T>>,
}

pub(crate) fn mount_indexed_list<'scope, IF, IS, T>(
    context: MountContext<'scope>,
    source: IF,
    factory: RowFactory<'scope, T>,
) -> SilexResult<MountInstance<'scope>>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    let local_owner = context.owner().child();
    let range = RangeHandle::at_target(context.target(), "for")?;
    let error_handler = context.error_handler();
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
    let state = local_owner.owner_state(IndexedState { rows: Vec::new() })?;
    let cleanup_state = state.clone();
    let cleanup_range = range.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let mut state = cleanup_state
                .take_for_cleanup()
                .unwrap_or(IndexedState { rows: Vec::new() });
            let error = dispose_rows(&mut state.rows);
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
            let new_len = values.len();
            let mut state = effect_state.take()?;
            let old_len = state.rows.len();
            let mut updated = Vec::new();
            let mut pending = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                for (index, item) in values.iter().cloned().enumerate().take(old_len) {
                    let snapshot = state.rows[index].snapshot();
                    state.rows[index].update(item, index)?;
                    updated.push((index, snapshot));
                }
                for (index, item) in values.into_iter().enumerate().skip(old_len) {
                    let row_range =
                        RangeHandle::before(&effect_context.dom().clone(), &end, "for-row")?;
                    pending.push(RowBlock::new(
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
                    )?);
                }
                Ok(())
            }));
            match result {
                Ok(Ok(())) => {
                    let mut removed = if new_len >= old_len {
                        Vec::new()
                    } else {
                        state.rows.split_off(new_len)
                    };
                    state.rows.append(&mut pending);
                    let error = dispose_rows(&mut removed);
                    effect_state.replace(state)?;
                    error.map_or(Ok(()), |error| {
                        Err(SilexError::fatal(SilexErrorKind::Close(error)))
                    })
                }
                Ok(Err(error)) => {
                    let restore = restore_indexed(&mut state.rows, &updated);
                    let cleanup = dispose_rows(&mut pending);
                    effect_state.replace(state)?;
                    Err(reconcile_rollback_error(error, None, restore, cleanup))
                }
                Err(panic) => {
                    let restore = restore_indexed(&mut state.rows, &updated);
                    let cleanup = dispose_rows(&mut pending);
                    effect_state.replace(state)?;
                    let error = SilexError::fatal(SilexErrorKind::Javascript(panic_message(
                        "Indexed list",
                        panic,
                    )));
                    Err(reconcile_rollback_error(error, None, restore, cleanup))
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
        error_handler,
    ) {
        let _ = local_owner.close();
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![range.start, range.end]))
}
