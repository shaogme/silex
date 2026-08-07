use super::owner::{DomRange, RowController, RowRender, RowRenderArgs, RowUpdater};
use crate::attribute::PendingAttribute;
use crate::view::{AnyView, ApplyAttributes, OwnedViewOwner, View, ViewOwner};
use silex_core::reactivity::{ReactiveSource, runtime_inputs_of};
use silex_core::traits::{ForLoopSource, RxRead};
use silex_core::{ErrorHandler, ErrorReporter, RuntimeInputs, SilexError, SilexResult};
use std::{
    cell::RefCell,
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
            self.error_handler.clone(),
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
    let local_owner = OwnedViewOwner::new(scope.clone(), owner.token().error_reporter());
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
    let rows = Rc::new(RefCell::new(Vec::<RowController<'scope, T>>::new()));

    let cleanup_rows = rows.clone();
    let cleanup_range = range.clone();
    let error_handler = local_owner.token().error_handler();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let mut rows = mem::take(&mut *cleanup_rows.borrow_mut());
            let panic = dispose_rows(&mut rows);
            cleanup_range.remove();
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
            Ok(())
        }),
        error_handler.clone(),
    ) {
        scope.dispose();
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
                .map_err(SilexError::from)
                .and_then(|result| result)?;
            let mut rows = mem::take(&mut *effect_rows.borrow_mut());
            let old_len = rows.len();
            let new_len = values.len();
            let mut pending = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                let mut values = values.into_iter();
                for (index, row) in rows.iter_mut().enumerate().take(new_len) {
                    let item = values.next().expect("snapshot length is stable");
                    row.update(item, index)?;
                }
                for (offset, item) in values.enumerate() {
                    let index = old_len + offset;
                    let row_range = DomRange::before(&end, "for-row")?;
                    let row = RowController::try_new(
                        &token,
                        row_range,
                        render.clone(),
                        RuntimeInputs::new(),
                        attrs.clone(),
                        item,
                        index,
                        stateful,
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
                    *effect_rows.borrow_mut() = rows;
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Ok(())
                }
                Ok(Err(error)) => {
                    let cleanup_panic = dispose_rows(&mut pending);
                    *effect_rows.borrow_mut() = rows;
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(error)
                }
                Err(panic) => {
                    let cleanup_panic = dispose_rows(&mut pending);
                    *effect_rows.borrow_mut() = rows;
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(panic_error("Indexed list", panic))
                }
            }
        }),
        error_handler,
    ) {
        scope.dispose();
        range.remove();
        return Err(error);
    }

    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            scope_for_cleanup.dispose();
            Ok(())
        }),
        owner.token().error_handler(),
    ) {
        scope.dispose();
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
    let reporter = error_handler
        .map(|handler| ErrorReporter::new(move |error| handler.handle(error)))
        .unwrap_or_else(|| owner.token().error_reporter());
    let local_owner = OwnedViewOwner::new(scope.clone(), reporter);
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
    let state = Rc::new(RefCell::new(KeyedRows {
        rows: HashMap::new(),
        order: Vec::new(),
    }));

    let cleanup_state = state.clone();
    let cleanup_range = range.clone();
    let effect_handler = local_owner.token().error_handler();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let mut state = cleanup_state.borrow_mut();
            let mut rows = mem::take(&mut state.rows).into_values().collect::<Vec<_>>();
            state.order.clear();
            drop(state);
            let panic = dispose_rows(&mut rows);
            cleanup_range.remove();
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
            Ok(())
        }),
        effect_handler.clone(),
    ) {
        scope.dispose();
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
                .map_err(SilexError::from)
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

            let mut old_rows = {
                let mut state = effect_state.borrow_mut();
                mem::take(&mut state.rows)
            };
            let old_order = {
                let mut state = effect_state.borrow_mut();
                mem::take(&mut state.order)
            };
            let mut pending = HashMap::with_capacity(keys.len());
            let mut seen = HashSet::with_capacity(keys.len());
            let mut next_order = Vec::with_capacity(keys.len());
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                for (index, (key, item)) in keys.iter().cloned().zip(values).enumerate() {
                    if let Some(row) = old_rows.get_mut(&key) {
                        row.update(item, index)?;
                        seen.insert(key.clone());
                        next_order.push(key);
                        continue;
                    }
                    let row_range = DomRange::before(&end, "for-row")?;
                    let row = RowController::try_new(
                        &token,
                        row_range,
                        render.clone(),
                        RuntimeInputs::new(),
                        attrs.clone(),
                        item,
                        index,
                        stateful,
                    )?;
                    seen.insert(key.clone());
                    next_order.push(key.clone());
                    pending.insert(key, row);
                }
                Ok(())
            }));

            match result {
                Ok(Ok(())) => {
                    let mut removed = Vec::new();
                    for key in &old_order {
                        if !seen.contains(key)
                            && let Some(row) = old_rows.remove(key)
                        {
                            removed.push(row);
                        }
                    }
                    old_rows.extend(pending.drain());
                    for key in &next_order {
                        if let Some(row) = old_rows.get(key) {
                            row.move_before(&end)?;
                        }
                    }
                    let cleanup_panic = dispose_rows(&mut removed);
                    let mut state = effect_state.borrow_mut();
                    state.rows = old_rows;
                    state.order = next_order;
                    drop(state);
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Ok(())
                }
                Ok(Err(error)) => {
                    restore_keyed_order(&old_rows, &old_order, &end);
                    let cleanup_panic = dispose_map(&mut pending);
                    let mut state = effect_state.borrow_mut();
                    state.rows = old_rows;
                    state.order = old_order;
                    drop(state);
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(error)
                }
                Err(panic) => {
                    restore_keyed_order(&old_rows, &old_order, &end);
                    let cleanup_panic = dispose_map(&mut pending);
                    let mut state = effect_state.borrow_mut();
                    state.rows = old_rows;
                    state.order = old_order;
                    drop(state);
                    if let Some(panic) = cleanup_panic {
                        resume_unwind(panic);
                    }
                    Err(panic_error("Keyed list", panic))
                }
            }
        }),
        effect_handler,
    ) {
        scope.dispose();
        return Err(error);
    }

    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            scope_for_cleanup.dispose();
            Ok(())
        }),
        owner.token().error_handler(),
    ) {
        scope.dispose();
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
