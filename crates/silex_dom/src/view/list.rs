use super::owner::{
    DomRange, RowController, RowControllerConfig, RowRender, RowRenderArgs, RowUpdater,
};
use crate::attribute::PendingAttribute;
use crate::view::{AnyView, ApplyAttributes, OwnedViewOwner, View, ViewOwner};
use silex_core::reactivity::{ReactiveSource, runtime_inputs_of};
use silex_core::traits::{ForLoopSource, RxRead};
use silex_core::{ErrorHandler, RuntimeInputs, SilexError, SilexResult};
use std::{
    collections::{HashMap, HashSet},
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use web_sys::Node;

/// Keyed list with persistent row controllers and state-preserving updates.
pub struct KeyedLoopView<'scope, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize, RowUpdater<'scope, T>) -> AnyView<'scope> + 'scope>,
    pub error_handler: Option<ErrorHandler<'scope, SilexError>>,
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

impl<'scope, IF, IS, T, K> ApplyAttributes<'scope> for KeyedLoopView<'scope, IF, IS, T, K> {}

impl<'scope, IF, IS, T, K> View<'scope> for KeyedLoopView<'scope, IF, IS, T, K>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        mount_keyed_list(
            owner,
            parent,
            self.each.clone(),
            self.key_fn.clone(),
            RowFactory::Stateful(self.view_fn.clone()),
            self.error_handler,
            attrs,
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_keyed_list(
            owner,
            parent,
            self.each,
            self.key_fn,
            RowFactory::Stateful(self.view_fn),
            self.error_handler,
            attrs,
        )
    }
}

pub struct IndexedLoopView<'scope, IF, T, IS> {
    pub each: IF,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    pub _marker: std::marker::PhantomData<(T, IS)>,
}

impl<'scope, IF, T, IS> ApplyAttributes<'scope> for IndexedLoopView<'scope, IF, T, IS> {}

impl<'scope, IF, T, IS> View<'scope> for IndexedLoopView<'scope, IF, T, IS>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        mount_indexed_list(
            owner,
            parent,
            self.each.clone(),
            RowFactory::RenderOnly(self.view_fn.clone()),
            attrs,
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_indexed_list(
            owner,
            parent,
            self.each,
            RowFactory::RenderOnly(self.view_fn),
            attrs,
        )
    }
}

fn mount_indexed_list<'scope, IF, IS, T>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    source: IF,
    factory: RowFactory<'scope, T>,
    attrs: Vec<PendingAttribute<'scope>>,
) -> SilexResult<()>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    let inputs = runtime_inputs_of(source.clone());
    owner.validate_inputs(&inputs)?;
    let scope = Rc::new(owner.try_owned_scope()?);
    let local_owner = OwnedViewOwner::new(scope.clone(), owner.token().error_handler());
    let range = DomRange::append(parent, "for")?;
    let token = local_owner.token();
    let stateful = factory.is_stateful();
    let render_factory = factory.clone();
    let render = RowRender::new(move |args: RowRenderArgs<'scope, T>| {
        let RowRenderArgs {
            item,
            index,
            parent,
            attrs,
            owner: token,
            updater,
        } = args;
        render_factory
            .render(item, index, updater)
            .mount_owned(&token, &parent, attrs)
    });
    let rows = local_owner
        .token()
        .owner_state(Vec::<RowController<'scope, T>>::new())?;

    let cleanup_rows = rows.clone();
    let cleanup_range = range.clone();
    let error_handler = local_owner.token().error_handler();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let mut rows = cleanup_rows.take_for_cleanup().unwrap_or_default();
            let panic = dispose_rows(&mut rows);
            cleanup_range.remove();
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
            Ok(())
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        range.remove();
        return Err(error);
    }

    let effect_rows = rows;
    let end = range.end.clone();
    if let Err(error) = local_owner.effect_from(
        inputs,
        Box::new(move || -> SilexResult<()> {
            let values = source
                .try_with(|items| items.as_slice().map(|values| values.to_vec()))
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
                    let row_range = DomRange::before(&end, "for-row")?;
                    let row = RowController::try_new(
                        &token,
                        RowControllerConfig {
                            range: row_range,
                            render: render.clone(),
                            render_inputs: RuntimeInputs::new(),
                            attrs: attrs.clone(),
                            item,
                            index,
                            stateful,
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
                    let cleanup_panic = dispose_rows(&mut removed);
                    effect_rows.replace(rows)?;
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Ok(())
                }
                Ok(Err(error)) => {
                    let restore_panic = restore_indexed_rows(&mut rows, &updated);
                    let cleanup_panic = dispose_rows(&mut pending);
                    effect_rows.replace(rows)?;
                    if let Some(panic) = restore_panic {
                        resume_unwind(panic);
                    }
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(error)
                }
                Err(panic) => {
                    let restore_panic = restore_indexed_rows(&mut rows, &updated);
                    let cleanup_panic = dispose_rows(&mut pending);
                    effect_rows.replace(rows)?;
                    if let Some(panic) = restore_panic {
                        resume_unwind(panic);
                    }
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(panic_error("Indexed list", panic))
                }
            }
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        range.remove();
        return Err(error);
    }

    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            let _ = scope_for_cleanup.dispose();
            Ok(())
        }),
        owner.token().error_handler(),
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    Ok(())
}

struct KeyedRows<'scope, T, K> {
    rows: HashMap<K, RowController<'scope, T>>,
    order: Vec<K>,
}

fn mount_keyed_list<'scope, IF, IS, T, K>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    source: IF,
    key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    factory: RowFactory<'scope, T>,
    error_handler: Option<ErrorHandler<'scope, SilexError>>,
    attrs: Vec<PendingAttribute<'scope>>,
) -> SilexResult<()>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
{
    let inputs = runtime_inputs_of(source.clone());
    owner.validate_inputs(&inputs)?;
    let scope = Rc::new(owner.try_owned_scope()?);
    let error_handler = error_handler.unwrap_or_else(|| owner.token().error_handler());
    let local_owner = OwnedViewOwner::new(scope.clone(), error_handler);
    let token = local_owner.token();
    let range = DomRange::append(parent, "for")?;
    let stateful = factory.is_stateful();
    let render_factory = factory.clone();
    let render = RowRender::new(move |args: RowRenderArgs<'scope, T>| {
        let RowRenderArgs {
            item,
            index,
            parent,
            attrs,
            owner: token,
            updater,
        } = args;
        render_factory
            .render(item, index, updater)
            .mount_owned(&token, &parent, attrs)
    });
    let state = local_owner.token().owner_state(KeyedRows {
        rows: HashMap::new(),
        order: Vec::new(),
    })?;

    let cleanup_state = state.clone();
    let cleanup_range = range.clone();
    let effect_handler = local_owner.token().error_handler();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let Some(mut state) = cleanup_state.take_for_cleanup() else {
                return Ok(());
            };
            let mut rows = mem::take(&mut state.rows).into_values().collect::<Vec<_>>();
            let panic = dispose_rows(&mut rows);
            cleanup_range.remove();
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
            Ok(())
        }),
        effect_handler,
    ) {
        let _ = scope.dispose();
        range.remove();
        return Err(error);
    }

    let effect_state = state;
    let end = range.end.clone();
    if let Err(error) = local_owner.effect_from(
        inputs,
        Box::new(move || -> SilexResult<()> {
            let values = source
                .try_with(|items| items.as_slice().map(|values| values.to_vec()))
                .and_then(|result| result)?;

            let key_result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<Vec<K>> {
                let mut keys = Vec::with_capacity(values.len());
                let mut seen = HashSet::with_capacity(values.len());
                for item in &values {
                    let key = key_fn(item);
                    if !seen.insert(key.clone()) {
                        return Err(SilexError::Framework(
                            "duplicate key in keyed list".to_string(),
                        ));
                    }
                    keys.push(key);
                }
                Ok(keys)
            }));
            let keys = match key_result {
                Ok(Ok(keys)) => keys,
                Ok(Err(error)) => return Err(error),
                Err(panic) => {
                    return Err(panic_error("Keyed list key function", panic));
                }
            };

            let mut state = effect_state.take()?;
            let mut old_rows = mem::take(&mut state.rows);
            let old_order = mem::take(&mut state.order);
            let mut pending = HashMap::with_capacity(keys.len());
            let mut seen = HashSet::with_capacity(keys.len());
            let mut next_order = Vec::with_capacity(keys.len());
            let mut updated = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                for (index, (key, item)) in keys.iter().cloned().zip(values).enumerate() {
                    if let Some(row) = old_rows.get_mut(&key) {
                        let previous = row.snapshot();
                        row.update(item, index)?;
                        updated.push((key.clone(), previous));
                        seen.insert(key.clone());
                        next_order.push(key);
                        continue;
                    }
                    let row_range = DomRange::before(&end, "for-row")?;
                    let row = RowController::try_new(
                        &token,
                        RowControllerConfig {
                            range: row_range,
                            render: render.clone(),
                            render_inputs: RuntimeInputs::new(),
                            attrs: attrs.clone(),
                            item,
                            index,
                            stateful,
                        },
                    )?;
                    seen.insert(key.clone());
                    next_order.push(key.clone());
                    pending.insert(key, row);
                }
                Ok(())
            }));

            let result = match result {
                Ok(Ok(())) => {
                    let move_result = (|| -> SilexResult<()> {
                        for key in &next_order {
                            if let Some(row) = old_rows.get(key) {
                                row.move_before(&end)?;
                            } else if let Some(row) = pending.get(key) {
                                row.move_before(&end)?;
                            } else {
                                return Err(SilexError::Framework(
                                    "keyed list row disappeared during diff".to_string(),
                                ));
                            }
                        }
                        Ok(())
                    })();
                    if let Err(error) = move_result {
                        restore_keyed_order(&old_rows, &old_order, &end);
                        let restore_panic = restore_keyed_rows(&mut old_rows, &updated);
                        let cleanup_panic = dispose_map(&mut pending);
                        state.rows = old_rows;
                        state.order = old_order;
                        if let Some(panic) = restore_panic {
                            resume_unwind(panic);
                        }
                        if let Some(panic) = cleanup_panic {
                            resume_unwind(panic);
                        }
                        return Err(error);
                    }

                    let mut removed = Vec::new();
                    for key in &old_order {
                        if !seen.contains(key)
                            && let Some(row) = old_rows.remove(key)
                        {
                            removed.push(row);
                        }
                    }
                    old_rows.extend(pending.drain());
                    let cleanup_panic = dispose_rows(&mut removed);
                    state.rows = old_rows;
                    state.order = next_order;
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Ok(())
                }
                Ok(Err(error)) => {
                    restore_keyed_order(&old_rows, &old_order, &end);
                    let restore_panic = restore_keyed_rows(&mut old_rows, &updated);
                    let cleanup_panic = dispose_map(&mut pending);
                    state.rows = old_rows;
                    state.order = old_order;
                    if let Some(panic) = restore_panic {
                        resume_unwind(panic);
                    }
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(error)
                }
                Err(panic) => {
                    restore_keyed_order(&old_rows, &old_order, &end);
                    let restore_panic = restore_keyed_rows(&mut old_rows, &updated);
                    let cleanup_panic = dispose_map(&mut pending);
                    state.rows = old_rows;
                    state.order = old_order;
                    if let Some(panic) = restore_panic {
                        resume_unwind(panic);
                    }
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(panic_error("Keyed list", panic))
                }
            };
            effect_state.replace(state)?;
            result
        }),
        effect_handler,
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
        owner.token().error_handler(),
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    Ok(())
}

fn dispose_map<'scope, T: Clone + 'scope, K>(
    rows: &mut HashMap<K, RowController<'scope, T>>,
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut values = rows.drain().map(|(_, row)| row).collect::<Vec<_>>();
    dispose_rows(&mut values)
}

fn restore_indexed_rows<'scope, T: Clone + 'scope>(
    rows: &mut [RowController<'scope, T>],
    updated: &[(usize, (T, usize))],
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut first_panic = None;
    for (index, (item, row_index)) in updated.iter().rev() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = rows[*index].update(item.clone(), *row_index);
        }));
        if let Err(panic) = result
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
    }
    first_panic
}

fn restore_keyed_rows<'scope, T: Clone + 'scope, K>(
    rows: &mut HashMap<K, RowController<'scope, T>>,
    updated: &[(K, (T, usize))],
) -> Option<Box<dyn std::any::Any + Send>>
where
    K: std::hash::Hash + Eq,
{
    let mut first_panic = None;
    for (key, (item, index)) in updated.iter().rev() {
        let Some(row) = rows.get_mut(key) else {
            continue;
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = row.update(item.clone(), *index);
        }));
        if let Err(panic) = result
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
    }
    first_panic
}

fn restore_keyed_order<'scope, T, K>(
    rows: &HashMap<K, RowController<'scope, T>>,
    order: &[K],
    end: &Node,
) where
    K: std::hash::Hash + Eq,
{
    for key in order {
        if let Some(row) = rows.get(key) {
            let _ = row.move_before(end);
        }
    }
}

fn panic_error(prefix: &str, panic: Box<dyn std::any::Any + Send>) -> SilexError {
    let message = if let Some(value) = panic.downcast_ref::<&str>() {
        format!("{prefix}: {value}")
    } else if let Some(value) = panic.downcast_ref::<String>() {
        format!("{prefix}: {value}")
    } else {
        format!("{prefix}: unknown panic")
    };
    SilexError::Javascript(message)
}

fn dispose_rows<'scope, T: Clone + 'scope>(
    rows: &mut Vec<RowController<'scope, T>>,
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut first_panic = None;
    for mut row in rows.drain(..) {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| row.dispose()))
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
    }
    first_panic
}
