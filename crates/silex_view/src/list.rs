use crate::any::AnyView;
use crate::context::{MountContext, MountTarget};
use crate::contract::{MountInstance, View};
use crate::owner::MountErrorHandler;
use crate::row::{
    RangeHandle, RowBlock, RowBlockConfig, RowRenderContext, RowRenderer, RowUpdater,
};
use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use silex_core::{
    CloseError, EffectPhase, ErrorHandlerToken, SilexError, SilexErrorKind, SilexResult,
};
use silex_dom::{diagnostics::DomError, model::DomNode};
use std::{
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

pub struct RenderOnlyKeyedListView<'scope, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    pub error_handler: Option<ErrorHandlerToken<'scope>>,
    pub _marker: std::marker::PhantomData<(IS, T)>,
}

pub struct StatefulKeyedListView<'scope, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>,
    pub error_handler: Option<ErrorHandlerToken<'scope>>,
    pub _marker: std::marker::PhantomData<(IS, T)>,
}

pub struct IndexedListView<'scope, IF, T, IS> {
    pub each: IF,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    pub _marker: std::marker::PhantomData<(T, IS)>,
}

enum RowFactory<'scope, T> {
    RenderOnly(Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>),
    Stateful(Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>),
}
impl<'scope, T> Clone for RowFactory<'scope, T> {
    fn clone(&self) -> Self {
        match self {
            Self::RenderOnly(value) => Self::RenderOnly(value.clone()),
            Self::Stateful(value) => Self::Stateful(value.clone()),
        }
    }
}
impl<'scope, T> RowFactory<'scope, T> {
    fn render(&self, item: T, index: usize, updater: RowUpdater<'scope, T>) -> AnyView<'scope> {
        match self {
            Self::RenderOnly(value) => value(item, index),
            Self::Stateful(value) => value(item, index, updater),
        }
    }
    fn stateful(&self) -> bool {
        matches!(self, Self::Stateful(_))
    }
}

impl<'scope, IF, IS, T, K> View<'scope> for RenderOnlyKeyedListView<'scope, IF, IS, T, K>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_keyed_list(KeyedListConfig {
            context: context.clone(),
            source: self.each.clone(),
            key_fn: self.key_fn.clone(),
            factory: RowFactory::RenderOnly(self.view_fn.clone()),
            custom_handler: self.error_handler.clone(),
            parent_handler: context.error_handler(),
        })
    }
}

impl<'scope, IF, IS, T, K> View<'scope> for StatefulKeyedListView<'scope, IF, IS, T, K>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_keyed_list(KeyedListConfig {
            context: context.clone(),
            source: self.each.clone(),
            key_fn: self.key_fn.clone(),
            factory: RowFactory::Stateful(self.view_fn.clone()),
            custom_handler: self.error_handler.clone(),
            parent_handler: context.error_handler(),
        })
    }
}

impl<'scope, IF, T, IS> View<'scope> for IndexedListView<'scope, IF, T, IS>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        mount_indexed_list(
            context.clone(),
            self.each.clone(),
            RowFactory::RenderOnly(self.view_fn.clone()),
        )
    }
}

fn read_values<IF, IS, T>(source: &IF) -> SilexResult<Vec<T>>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS>,
    IS: ForLoopSource<Item = T>,
    T: Clone,
{
    source
        .with(|items| items.as_slice().map(|values| values.to_vec()))
        .and_then(|result| result)
}

struct IndexedState<'scope, T> {
    rows: Vec<RowBlock<'scope, T>>,
}

fn mount_indexed_list<'scope, IF, IS, T>(
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

struct KeyedRow<'scope, T, K> {
    row: RowBlock<'scope, T>,
    marker: std::marker::PhantomData<K>,
}
struct KeyedState<'scope, T, K> {
    rows: HashMap<K, KeyedRow<'scope, T, K>>,
    order: Vec<K>,
}

struct KeyedListConfig<'scope, IF, T, K> {
    context: MountContext<'scope>,
    source: IF,
    key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    factory: RowFactory<'scope, T>,
    custom_handler: Option<ErrorHandlerToken<'scope>>,
    parent_handler: MountErrorHandler<'scope>,
}

fn mount_keyed_list<'scope, IF, IS, T, K>(
    config: KeyedListConfig<'scope, IF, T, K>,
) -> SilexResult<MountInstance<'scope>>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
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
                                marker: std::marker::PhantomData,
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

fn restore_keyed_order<T, K>(
    rows: &mut HashMap<K, KeyedRow<'_, T, K>>,
    order: &[K],
    parent: &DomNode,
    end: &DomNode,
) -> SilexResult<()>
where
    K: std::hash::Hash + Eq,
{
    for key in order {
        if let Some(row) = rows.get_mut(key) {
            row.row.move_before(parent, end)?;
        }
    }
    Ok(())
}

fn reconcile_rollback_error(
    primary: SilexError,
    restore_order: Option<SilexError>,
    restore_updates: Option<CloseError>,
    cleanup: Option<CloseError>,
) -> SilexError {
    if restore_order.is_none() && restore_updates.is_none() && cleanup.is_none() {
        return primary;
    }
    SilexError::fatal(SilexErrorKind::Framework(format!(
        "keyed reconcile rollback failed after {primary}; order={restore_order:?}; updates={restore_updates:?}; cleanup={cleanup:?}"
    )))
}

fn dispose_rows<'scope, T>(rows: &mut Vec<RowBlock<'scope, T>>) -> Option<CloseError> {
    let mut errors = Vec::new();
    for mut row in rows.drain(..) {
        match catch_unwind(AssertUnwindSafe(|| row.dispose())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(error),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}
fn dispose_keyed<'scope, T, K>(rows: &mut Vec<KeyedRow<'scope, T, K>>) -> Option<CloseError> {
    let mut errors = Vec::new();
    for KeyedRow { mut row, .. } in rows.drain(..) {
        match catch_unwind(AssertUnwindSafe(|| row.dispose())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(error),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}
fn restore_indexed<'scope, T: Clone + 'scope>(
    rows: &mut [RowBlock<'scope, T>],
    updates: &[(usize, (T, usize))],
) -> Option<CloseError> {
    let mut errors = Vec::new();
    for (index, (item, row_index)) in updates.iter().rev() {
        match catch_unwind(AssertUnwindSafe(|| {
            rows[*index].update(item.clone(), *row_index)
        })) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(CloseError::from_panic(Box::new(error.to_string()))),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}
fn restore_keyed<'scope, T: Clone + 'scope, K: std::hash::Hash + Eq>(
    rows: &mut HashMap<K, KeyedRow<'scope, T, K>>,
    updates: &[(K, (T, usize))],
) -> Option<CloseError> {
    let mut errors = Vec::new();
    for (key, (item, index)) in updates.iter().rev() {
        if let Some(row) = rows.get_mut(key) {
            match catch_unwind(AssertUnwindSafe(|| row.row.update(item.clone(), *index))) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => errors.push(CloseError::from_panic(Box::new(error.to_string()))),
                Err(panic) => errors.push(CloseError::from_panic(panic)),
            }
        }
    }
    CloseError::combine(errors)
}
fn panic_message(prefix: &str, panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(value) = panic.downcast_ref::<&str>() {
        format!("{prefix}: {value}")
    } else if let Some(value) = panic.downcast_ref::<String>() {
        format!("{prefix}: {value}")
    } else {
        format!("{prefix}: unknown panic")
    }
}
