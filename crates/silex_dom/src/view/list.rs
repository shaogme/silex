use super::owner::MountOwner;
use super::row::{
    NodeRange, RowInstance, RowInstanceConfig, RowRenderContext, RowRenderer, RowUpdater,
};
use crate::attribute::AttrOp;
use crate::view::{AnyView, ApplyAttributes, MountErrorHandler, MountInstance, View};
use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead};
use silex_core::{CloseError, ErrorHandlerToken, SilexError, SilexErrorKind, SilexResult};
use std::{
    collections::{HashMap, HashSet},
    mem,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use web_sys::Node;

/// Keyed list that re-renders each row when its item or index changes.
pub struct RenderOnlyKeyedListView<'scope, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    pub error_handler: Option<ErrorHandlerToken<'scope>>,
    pub _marker: std::marker::PhantomData<(IS, T)>,
}

/// Keyed list with persistent row controllers and state-preserving updates.
pub struct StatefulKeyedListView<'scope, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>,
    pub error_handler: Option<ErrorHandlerToken<'scope>>,
    pub _marker: std::marker::PhantomData<(IS, T)>,
}

enum RowFactory<'scope, T> {
    RenderOnly(Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>),
    Stateful(Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>),
}

impl<'scope, T> Clone for RowFactory<'scope, T> {
    fn clone(&self) -> Self {
        match self {
            Self::RenderOnly(factory) => Self::RenderOnly(factory.clone()),
            Self::Stateful(factory) => Self::Stateful(factory.clone()),
        }
    }
}

impl<'scope, T> RowFactory<'scope, T> {
    fn render(&self, item: T, index: usize, updater: RowUpdater<'scope, T>) -> AnyView<'scope> {
        match self {
            Self::RenderOnly(factory) => factory(item, index),
            Self::Stateful(factory) => factory(item, index, updater),
        }
    }

    fn is_stateful(&self) -> bool {
        matches!(self, Self::Stateful(_))
    }
}

impl<'scope, IF, IS, T, K> ApplyAttributes<'scope>
    for RenderOnlyKeyedListView<'scope, IF, IS, T, K>
{
}

impl<'scope, IF, IS, T, K> View<'scope> for RenderOnlyKeyedListView<'scope, IF, IS, T, K>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_keyed_list(KeyedListMountArgs {
            owner,
            parent,
            source: self.each.clone(),
            key_fn: self.key_fn.clone(),
            factory: RowFactory::RenderOnly(self.view_fn.clone()),
            error_handler: self.error_handler.clone(),
            attrs,
            parent_error_handler: error_handler,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'scope, IF, IS, T, K> ApplyAttributes<'scope> for StatefulKeyedListView<'scope, IF, IS, T, K> {}

impl<'scope, IF, IS, T, K> View<'scope> for StatefulKeyedListView<'scope, IF, IS, T, K>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_keyed_list(KeyedListMountArgs {
            owner,
            parent,
            source: self.each.clone(),
            key_fn: self.key_fn.clone(),
            factory: RowFactory::Stateful(self.view_fn.clone()),
            error_handler: self.error_handler.clone(),
            attrs,
            parent_error_handler: error_handler,
            _marker: std::marker::PhantomData,
        })
    }
}

pub struct IndexedListView<'scope, IF, T, IS> {
    pub each: IF,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    pub _marker: std::marker::PhantomData<(T, IS)>,
}

impl<'scope, IF, T, IS> ApplyAttributes<'scope> for IndexedListView<'scope, IF, T, IS> {}

impl<'scope, IF, T, IS> View<'scope> for IndexedListView<'scope, IF, T, IS>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_indexed_list(
            owner,
            parent,
            self.each.clone(),
            RowFactory::RenderOnly(self.view_fn.clone()),
            attrs,
            error_handler,
        )
    }
}

fn mount_indexed_list<'scope, IF, IS, T>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    source: IF,
    factory: RowFactory<'scope, T>,
    attrs: Vec<AttrOp<'scope>>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<MountInstance<'scope>>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    let local_owner = owner.child();
    let range = NodeRange::append(parent, "for")?;
    let token = local_owner.token();
    let stateful = factory.is_stateful();
    let render_factory = factory.clone();
    let render = RowRenderer::new(move |args: RowRenderContext<'scope, T>| {
        let RowRenderContext {
            item,
            index,
            parent,
            attrs,
            owner: token,
            branch_context: _,
            error_handler,
            updater,
        } = args;
        render_factory
            .render(item, index, updater)
            .mount(&token, &parent, attrs, error_handler)
            .map(|_| ())
    });
    let rows = local_owner
        .token()
        .owner_state(Vec::<RowInstance<'scope, T>>::new())?;

    let cleanup_rows = rows.clone();
    let cleanup_range = range.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let mut rows = cleanup_rows.take_for_cleanup().unwrap_or_default();
            let close_error = dispose_rows(&mut rows);
            cleanup_range.remove();
            close_error.map_or(Ok(()), |error| {
                Err(SilexError::fatal(SilexErrorKind::Close(error)))
            })
        }),
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        range.remove();
        return Err(error);
    }

    let effect_rows = rows;
    let end = range.end.clone();
    if let Err(error) = local_owner.effect(
        Box::new(move || -> SilexResult<()> {
            let values = source
                .with(|items| items.as_slice().map(|values| values.to_vec()))
                .and_then(|result| result)?;
            let mut rows = effect_rows.take()?;
            let old_len = rows.len();
            let new_len = values.len();
            let mut pending = Vec::new();
            let mut updated = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                let mut values = values.into_iter();
                for (row_index, row) in rows.iter_mut().enumerate().take(new_len) {
                    let item = values.next().expect("snapshot length is stable");
                    let previous = row.snapshot();
                    row.update(item, row_index)?;
                    updated.push((row_index, previous));
                }
                for (offset, item) in values.enumerate() {
                    let index = old_len + offset;
                    let row_range = NodeRange::before(&end, "for-row")?;
                    let row = RowInstance::new(
                        &token,
                        RowInstanceConfig {
                            range: row_range,
                            render: render.clone(),
                            attrs: attrs.clone(),
                            item,
                            index,
                            stateful,
                            branch_runtime: false,
                            error_handler,
                        },
                    )?;
                    pending.push(row);
                }
                Ok(())
            }));

            match result {
                Ok(Ok(())) => {
                    let mut removed = if new_len < old_len {
                        rows.split_off(new_len)
                    } else {
                        Vec::new()
                    };
                    rows.append(&mut pending);
                    let cleanup_error = dispose_rows(&mut removed);
                    effect_rows.replace(rows)?;
                    cleanup_error.map_or(Ok(()), |error| {
                        Err(SilexError::fatal(SilexErrorKind::Close(error)))
                    })
                }
                Ok(Err(error)) => {
                    let restore_error = restore_indexed_rows(&mut rows, &updated);
                    let cleanup_error = dispose_rows(&mut pending);
                    effect_rows.replace(rows)?;
                    if let Some(close_error) = combine_close_errors(restore_error, cleanup_error) {
                        report_close_failure(error_handler, close_error);
                    }
                    Err(error)
                }
                Err(panic) => {
                    let restore_error = restore_indexed_rows(&mut rows, &updated);
                    let cleanup_error = dispose_rows(&mut pending);
                    effect_rows.replace(rows)?;
                    if let Some(close_error) = combine_close_errors(restore_error, cleanup_error) {
                        report_close_failure(error_handler, close_error);
                    }
                    Err(SilexError::fatal(SilexErrorKind::Javascript(
                        panic_message("Indexed list", panic),
                    )))
                }
            }
        }),
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        range.remove();
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
    Ok(MountInstance::from_nodes(vec![range.start, range.end]))
}

#[derive(Clone, Copy)]
enum KeyedRowPhase {
    Pending,
    Active,
}

/// A keyed row keeps identity separate from the row runtime state. The
/// `RowInstance` owns the logical item/index, physical range, row/render
/// owners, and updater generation; this wrapper adds the key and transaction
/// phase used by keyed diffing.
struct KeyedRow<'scope, T, K> {
    key: K,
    row: RowInstance<'scope, T>,
    phase: KeyedRowPhase,
}

impl<'scope, T: Clone + 'scope, K> KeyedRow<'scope, T, K> {
    fn pending(key: K, row: RowInstance<'scope, T>) -> Self {
        Self {
            key,
            row,
            phase: KeyedRowPhase::Pending,
        }
    }

    fn activate(&mut self) {
        self.phase = KeyedRowPhase::Active;
    }

    fn snapshot(&self) -> (T, usize)
    where
        T: Clone,
    {
        self.row.snapshot()
    }

    fn update(&mut self, item: T, index: usize) -> SilexResult<()> {
        self.row.update(item, index)
    }

    fn append_to(&self, target: &Node) -> SilexResult<()> {
        self.row.append_to(target)
    }

    fn dispose(&mut self) -> Result<(), CloseError> {
        self.row.dispose()
    }
}

struct KeyedRows<'scope, T, K> {
    rows: HashMap<K, KeyedRow<'scope, T, K>>,
    order: Vec<K>,
}

enum KeyedRowOperation<K, T> {
    Reuse { key: K, item: T, index: usize },
    Insert { key: K, item: T, index: usize },
}

struct KeyedDiffPlan<K, T> {
    operations: Vec<KeyedRowOperation<K, T>>,
    next_order: Vec<K>,
    removed: Vec<K>,
}

fn commit_keyed_order<'scope, T: Clone + 'scope, K>(
    rows: &HashMap<K, KeyedRow<'scope, T, K>>,
    pending: &HashMap<K, KeyedRow<'scope, T, K>>,
    old_order: &[K],
    next_order: &[K],
    end: &Node,
) -> SilexResult<()>
where
    K: std::hash::Hash + Eq,
{
    let Some(parent) = end.parent_node() else {
        return Err(SilexError::fatal(SilexErrorKind::Dom(
            "cannot commit keyed order without a parent".to_string(),
        )));
    };
    let document = crate::document();
    let ordered: Node = document.create_document_fragment().into();
    let removed: Node = document.create_document_fragment().into();

    for key in next_order {
        if !rows.contains_key(key) && !pending.contains_key(key) {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "keyed list row disappeared during commit".to_string(),
            )));
        }
    }

    for key in old_order {
        if !next_order.iter().any(|next_key| next_key == key) {
            let row = rows.get(key).ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Framework(
                    "keyed list removed row disappeared during commit".to_string(),
                ))
            })?;
            row.append_to(&removed)?;
        }
    }

    for key in next_order {
        if let Some(row) = rows.get(key) {
            row.append_to(&ordered)?;
        } else if let Some(row) = pending.get(key) {
            row.append_to(&ordered)?;
        }
    }

    parent
        .insert_before(&ordered, Some(end))
        .map(|_| {
            drop(removed);
        })
        .map_err(SilexError::fatal)
}

fn plan_keyed_update<T, K>(
    old_rows: &HashMap<K, KeyedRow<'_, T, K>>,
    old_order: &[K],
    keys: Vec<K>,
    values: Vec<T>,
) -> SilexResult<KeyedDiffPlan<K, T>>
where
    K: std::hash::Hash + Eq + Clone,
{
    let mut operations = Vec::with_capacity(keys.len());
    let mut next_order = Vec::with_capacity(keys.len());
    let mut seen = HashSet::with_capacity(keys.len());
    for (index, (key, item)) in keys.into_iter().zip(values).enumerate() {
        if !seen.insert(key.clone()) {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "duplicate key in keyed list".to_string(),
            )));
        }
        let operation = if old_rows.contains_key(&key) {
            KeyedRowOperation::Reuse {
                key: key.clone(),
                item,
                index,
            }
        } else {
            KeyedRowOperation::Insert {
                key: key.clone(),
                item,
                index,
            }
        };
        operations.push(operation);
        next_order.push(key);
    }
    let removed = old_order
        .iter()
        .filter(|key| !seen.contains(*key))
        .cloned()
        .collect();
    Ok(KeyedDiffPlan {
        operations,
        next_order,
        removed,
    })
}

struct KeyedListMountArgs<'owner, 'scope, IF, IS, T, K> {
    owner: &'owner dyn MountOwner<'scope>,
    parent: &'owner Node,
    source: IF,
    key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    factory: RowFactory<'scope, T>,
    error_handler: Option<ErrorHandlerToken<'scope>>,
    attrs: Vec<AttrOp<'scope>>,
    parent_error_handler: MountErrorHandler<'scope>,
    _marker: std::marker::PhantomData<IS>,
}

fn mount_keyed_list<'owner, 'scope, IF, IS, T, K>(
    args: KeyedListMountArgs<'owner, 'scope, IF, IS, T, K>,
) -> SilexResult<MountInstance<'scope>>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
{
    let KeyedListMountArgs {
        owner,
        parent,
        source,
        key_fn,
        factory,
        error_handler,
        attrs,
        parent_error_handler,
        ..
    } = args;
    let local_owner = owner.child();
    let error_handler = error_handler
        .map(|handler| handler.view())
        .unwrap_or(parent_error_handler);
    let token = local_owner.token();
    let range = NodeRange::append(parent, "for")?;
    let stateful = factory.is_stateful();
    let render_factory = factory.clone();
    let render = RowRenderer::new(move |args: RowRenderContext<'scope, T>| {
        let RowRenderContext {
            item,
            index,
            parent,
            attrs,
            owner: token,
            branch_context: _,
            error_handler,
            updater,
        } = args;
        render_factory
            .render(item, index, updater)
            .mount(&token, &parent, attrs, error_handler)
            .map(|_| ())
    });
    let state = local_owner.token().owner_state(KeyedRows {
        rows: HashMap::new(),
        order: Vec::new(),
    })?;

    let cleanup_state = state.clone();
    let cleanup_range = range.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let Some(mut state) = cleanup_state.take_for_cleanup() else {
                return Ok(());
            };
            let mut rows = mem::take(&mut state.rows).into_values().collect::<Vec<_>>();
            let close_error = dispose_keyed_rows(&mut rows);
            cleanup_range.remove();
            close_error.map_or(Ok(()), |error| {
                Err(SilexError::fatal(SilexErrorKind::Close(error)))
            })
        }),
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        range.remove();
        return Err(error);
    }

    let effect_state = state;
    let end = range.end.clone();
    if let Err(error) = local_owner.effect(
        Box::new(move || -> SilexResult<()> {
            let values = source
                .with(|items| items.as_slice().map(|values| values.to_vec()))
                .and_then(|result| result)?;

            let key_result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<Vec<K>> {
                let mut keys = Vec::with_capacity(values.len());
                let mut seen = HashSet::with_capacity(values.len());
                for item in &values {
                    let key = key_fn(item);
                    if !seen.insert(key.clone()) {
                        return Err(SilexError::fatal(SilexErrorKind::Framework(
                            "duplicate key in keyed list".to_string(),
                        )));
                    }
                    keys.push(key);
                }
                Ok(keys)
            }));
            let keys = match key_result {
                Ok(Ok(keys)) => keys,
                Ok(Err(error)) => return Err(error),
                Err(panic) => {
                    return Err(SilexError::fatal(SilexErrorKind::Javascript(
                        panic_message("Keyed list key function", panic),
                    )));
                }
            };

            let mut state = effect_state.take()?;
            let mut old_rows = mem::take(&mut state.rows);
            let old_order = mem::take(&mut state.order);
            let plan = match plan_keyed_update(&old_rows, &old_order, keys, values) {
                Ok(plan) => plan,
                Err(error) => {
                    state.rows = old_rows;
                    state.order = old_order;
                    effect_state.replace(state)?;
                    return Err(error);
                }
            };
            let mut pending = HashMap::with_capacity(plan.operations.len());
            let mut updated = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                for operation in &plan.operations {
                    if let KeyedRowOperation::Reuse { key, item, index } = operation {
                        let Some(row) = old_rows.get_mut(key) else {
                            return Err(SilexError::fatal(SilexErrorKind::Framework(
                                "keyed list reused row disappeared during planning".to_string(),
                            )));
                        };
                        let previous = row.snapshot();
                        row.update(item.clone(), *index)?;
                        updated.push((key.clone(), previous));
                        continue;
                    }
                    let KeyedRowOperation::Insert { key, item, index } = operation else {
                        continue;
                    };
                    let row_range = NodeRange::detached("for-row")?;
                    let row = RowInstance::new(
                        &token,
                        RowInstanceConfig {
                            range: row_range,
                            render: render.clone(),
                            attrs: attrs.clone(),
                            item: item.clone(),
                            index: *index,
                            stateful,
                            branch_runtime: false,
                            error_handler,
                        },
                    )?;
                    let keyed_row = KeyedRow::pending(key.clone(), row);
                    pending.insert(keyed_row.key.clone(), keyed_row);
                }
                Ok(())
            }));

            let result = match result {
                Ok(Ok(())) => {
                    let move_result =
                        commit_keyed_order(&old_rows, &pending, &old_order, &plan.next_order, &end);
                    if let Err(error) = move_result {
                        restore_keyed_order(&old_rows, &old_order, &end);
                        let restore_error = restore_keyed_rows(&mut old_rows, &updated);
                        let cleanup_error = dispose_map(&mut pending);
                        state.rows = old_rows;
                        state.order = old_order;
                        if let Some(close_error) =
                            combine_close_errors(restore_error, cleanup_error)
                        {
                            report_close_failure(error_handler, close_error);
                        }
                        return Err(error);
                    }

                    let mut removed = Vec::new();
                    for key in &plan.removed {
                        if let Some(row) = old_rows.remove(key) {
                            removed.push(row);
                        }
                    }
                    for (key, mut row) in pending.drain() {
                        row.activate();
                        old_rows.insert(key, row);
                    }
                    let cleanup_error = dispose_keyed_rows(&mut removed);
                    state.rows = old_rows;
                    state.order = plan.next_order;
                    cleanup_error.map_or(Ok(()), |error| {
                        Err(SilexError::fatal(SilexErrorKind::Close(error)))
                    })
                }
                Ok(Err(error)) => {
                    restore_keyed_order(&old_rows, &old_order, &end);
                    let restore_error = restore_keyed_rows(&mut old_rows, &updated);
                    let cleanup_error = dispose_map(&mut pending);
                    state.rows = old_rows;
                    state.order = old_order;
                    if let Some(close_error) = combine_close_errors(restore_error, cleanup_error) {
                        report_close_failure(error_handler, close_error);
                    }
                    Err(error)
                }
                Err(panic) => {
                    restore_keyed_order(&old_rows, &old_order, &end);
                    let restore_error = restore_keyed_rows(&mut old_rows, &updated);
                    let cleanup_error = dispose_map(&mut pending);
                    state.rows = old_rows;
                    state.order = old_order;
                    if let Some(close_error) = combine_close_errors(restore_error, cleanup_error) {
                        report_close_failure(error_handler, close_error);
                    }
                    Err(SilexError::fatal(SilexErrorKind::Javascript(
                        panic_message("Keyed list", panic),
                    )))
                }
            };
            effect_state.replace(state)?;
            result
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
        parent_error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![range.start, range.end]))
}

fn dispose_map<'scope, T: Clone + 'scope, K>(
    rows: &mut HashMap<K, KeyedRow<'scope, T, K>>,
) -> Option<CloseError> {
    let mut values = rows.drain().map(|(_, row)| row).collect::<Vec<_>>();
    dispose_keyed_rows(&mut values)
}

fn restore_indexed_rows<'scope, T: Clone + 'scope>(
    rows: &mut [RowInstance<'scope, T>],
    updated: &[(usize, (T, usize))],
) -> Option<CloseError> {
    let mut errors = Vec::new();
    for (index, (item, row_index)) in updated.iter().rev() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            rows[*index].update(item.clone(), *row_index)
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(CloseError::from_panic(Box::new(format!(
                "indexed row rollback failed: {error}"
            )))),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}

fn restore_keyed_rows<'scope, T: Clone + 'scope, K>(
    rows: &mut HashMap<K, KeyedRow<'scope, T, K>>,
    updated: &[(K, (T, usize))],
) -> Option<CloseError>
where
    K: std::hash::Hash + Eq,
{
    let mut errors = Vec::new();
    for (key, (item, index)) in updated.iter().rev() {
        let Some(row) = rows.get_mut(key) else {
            continue;
        };
        let result = catch_unwind(AssertUnwindSafe(|| row.update(item.clone(), *index)));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(CloseError::from_panic(Box::new(format!(
                "keyed row rollback failed: {error}"
            )))),
            Err(panic) => errors.push(CloseError::from_panic(panic)),
        }
    }
    CloseError::combine(errors)
}

fn restore_keyed_order<'scope, T: Clone + 'scope, K>(
    rows: &HashMap<K, KeyedRow<'scope, T, K>>,
    order: &[K],
    end: &Node,
) where
    K: std::hash::Hash + Eq,
{
    let fragment: Node = crate::document().create_document_fragment().into();
    for key in order {
        if let Some(row) = rows.get(key) {
            let _ = row.append_to(&fragment);
        }
    }
    if let Some(parent) = end.parent_node() {
        let _ = parent.insert_before(&fragment, Some(end));
    }
}

fn combine_close_errors(
    first: Option<CloseError>,
    second: Option<CloseError>,
) -> Option<CloseError> {
    CloseError::combine(first.into_iter().chain(second))
}

fn report_close_failure(error_handler: MountErrorHandler<'_>, error: CloseError) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        error_handler.handle(SilexError::fatal(SilexErrorKind::Close(error)))
    }));
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

fn dispose_rows<'scope, T: Clone + 'scope>(
    rows: &mut Vec<RowInstance<'scope, T>>,
) -> Option<CloseError> {
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

fn dispose_keyed_rows<'scope, T: Clone + 'scope, K>(
    rows: &mut Vec<KeyedRow<'scope, T, K>>,
) -> Option<CloseError> {
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
