use super::owner::{DomRange, RowController, RowRender, RowRenderArgs, RowUpdater};
use crate::attribute::PendingAttribute;
use crate::view::{AnyView, ApplyAttributes, View, ViewOwner};
use silex_core::reactivity::{ReactiveSource, runtime_inputs_of};
use silex_core::traits::{ForErrorHandler, ForLoopSource, RxRead};
use silex_core::{RuntimeInputs, SilexError};
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
    pub error: ForErrorHandler,
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
    ) {
        mount_keyed_list(
            owner,
            parent,
            self.each.clone(),
            self.key_fn.clone(),
            RowFactory::Stateful(self.view_fn.clone()),
            self.error.clone(),
            attrs,
        );
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        mount_keyed_list(
            owner,
            parent,
            self.each,
            self.key_fn,
            RowFactory::Stateful(self.view_fn),
            self.error,
            attrs,
        );
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
    ) {
        mount_indexed_list(
            owner,
            parent,
            self.each.clone(),
            RowFactory::RenderOnly(self.view_fn.clone()),
            attrs,
        );
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        mount_indexed_list(
            owner,
            parent,
            self.each,
            RowFactory::RenderOnly(self.view_fn),
            attrs,
        );
    }
}

fn mount_indexed_list<'scope, IF, IS, T>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    source: IF,
    factory: RowFactory<'scope, T>,
    attrs: Vec<PendingAttribute<'scope>>,
) where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    let inputs = runtime_inputs_of(source.clone());
    if let Err(error) = owner.validate_inputs(&inputs) {
        owner.report_error(error);
        return;
    }
    let token = owner.token();
    let range = match DomRange::append(parent, "for") {
        Ok(range) => range,
        Err(error) => {
            owner.report_error(error);
            return;
        }
    };
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
            .mount_owned(&token, &parent, attrs);
    });
    let rows = Rc::new(RefCell::new(Vec::<RowController<'scope, T>>::new()));

    let cleanup_rows = rows.clone();
    let cleanup_range = range.clone();
    owner.on_cleanup(Box::new(move || {
        let mut rows = mem::take(&mut *cleanup_rows.borrow_mut());
        let panic = dispose_rows(&mut rows);
        cleanup_range.remove();
        if let Some(panic) = panic {
            resume_unwind(panic);
        }
    }));

    let effect_rows = rows;
    let end = range.end.clone();
    owner.effect_from(
        inputs,
        Box::new(move || {
            let snapshot = source
                .try_with(|items| items.as_slice().map(|values| values.to_vec()))
                .map_err(SilexError::from)
                .and_then(|snapshot| snapshot);
            match snapshot {
                Ok(values) => {
                    let mut rows = mem::take(&mut *effect_rows.borrow_mut());
                    let old_len = rows.len();
                    let new_len = values.len();
                    let mut pending = Vec::new();
                    let result = catch_unwind(AssertUnwindSafe(|| -> bool {
                        let mut values = values.into_iter();
                        for (index, row) in rows.iter_mut().enumerate().take(new_len) {
                            let item = values.next().expect("snapshot length is stable");
                            if !row.update(item, index) {
                                return false;
                            }
                        }
                        for (offset, item) in values.enumerate() {
                            let index = old_len + offset;
                            let Ok(row_range) = DomRange::before(&end, "for-row") else {
                                return false;
                            };
                            let Some(row) = RowController::try_new(
                                &token,
                                row_range,
                                render.clone(),
                                RuntimeInputs::new(),
                                attrs.clone(),
                                item,
                                index,
                                stateful,
                            ) else {
                                return false;
                            };
                            pending.push(row);
                        }
                        true
                    }));

                    match result {
                        Ok(true) => {
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
                        }
                        Ok(false) => {
                            let _ = dispose_rows(&mut pending);
                            *effect_rows.borrow_mut() = rows;
                            token.report_error(SilexError::Javascript(
                                "indexed row update was rejected".to_string(),
                            ));
                        }
                        Err(panic) => {
                            let mut pending_panic = dispose_rows(&mut pending);
                            let mut current = rows;
                            let current_panic = dispose_rows(&mut current);
                            if pending_panic.is_none() {
                                pending_panic = current_panic;
                            }
                            *effect_rows.borrow_mut() = Vec::new();
                            token.report_error(panic_error("Indexed list", panic));
                            drop(pending_panic);
                        }
                    }
                }
                Err(error) => {
                    let mut rows = mem::take(&mut *effect_rows.borrow_mut());
                    let panic = dispose_rows(&mut rows);
                    if let Some(panic) = panic {
                        resume_unwind(panic);
                    }
                    token.report_error(error);
                }
            }
        }),
    );
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
    error: ForErrorHandler,
    attrs: Vec<PendingAttribute<'scope>>,
) where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
{
    let inputs = runtime_inputs_of(source.clone());
    if let Err(error) = owner.validate_inputs(&inputs) {
        owner.report_error(error);
        return;
    }
    let token = owner.token();
    let range = match DomRange::append(parent, "for") {
        Ok(range) => range,
        Err(error) => {
            owner.report_error(error);
            return;
        }
    };
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
            .mount_owned(&token, &parent, attrs);
    });
    let state = Rc::new(RefCell::new(KeyedRows {
        rows: HashMap::new(),
        order: Vec::new(),
    }));

    let cleanup_state = state.clone();
    let cleanup_range = range.clone();
    owner.on_cleanup(Box::new(move || {
        let mut state = cleanup_state.borrow_mut();
        let mut rows = mem::take(&mut state.rows).into_values().collect::<Vec<_>>();
        state.order.clear();
        drop(state);
        let panic = dispose_rows(&mut rows);
        cleanup_range.remove();
        if let Some(panic) = panic {
            resume_unwind(panic);
        }
    }));

    let effect_state = state;
    let end = range.end.clone();
    owner.effect_from(
        inputs,
        Box::new(move || {
            let snapshot = source
                .try_with(|items| items.as_slice().map(|values| values.to_vec()))
                .map_err(SilexError::from)
                .and_then(|snapshot| snapshot);
            let values = match snapshot {
                Ok(values) => values,
                Err(source_error) => {
                    error.call(source_error);
                    return;
                }
            };

            let key_result = catch_unwind(AssertUnwindSafe(|| {
                let mut keys = Vec::with_capacity(values.len());
                let mut seen = HashSet::with_capacity(values.len());
                for item in &values {
                    let key = key_fn(item);
                    if !seen.insert(key.clone()) {
                        error.call(SilexError::Framework(
                            "duplicate key in keyed list".to_string(),
                        ));
                        return None;
                    }
                    keys.push(key);
                }
                Some(keys)
            }));
            let Some(keys) = (match key_result {
                Ok(keys) => keys,
                Err(panic) => {
                    report_panic(&error, "Keyed list key function", panic);
                    return;
                }
            }) else {
                return;
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
            let result = catch_unwind(AssertUnwindSafe(|| -> bool {
                for (index, (key, item)) in keys.iter().cloned().zip(values).enumerate() {
                    if let Some(row) = old_rows.get_mut(&key) {
                        if !row.update(item, index) {
                            return false;
                        }
                        seen.insert(key.clone());
                        next_order.push(key);
                        continue;
                    }
                    let Ok(row_range) = DomRange::before(&end, "for-row") else {
                        return false;
                    };
                    let Some(row) = RowController::try_new(
                        &token,
                        row_range,
                        render.clone(),
                        RuntimeInputs::new(),
                        attrs.clone(),
                        item,
                        index,
                        stateful,
                    ) else {
                        return false;
                    };
                    seen.insert(key.clone());
                    next_order.push(key.clone());
                    pending.insert(key, row);
                }
                true
            }));

            match result {
                Ok(true) => {
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
                            row.move_before(&end);
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
                }
                Ok(false) => {
                    let _ = dispose_map(&mut pending);
                    let mut state = effect_state.borrow_mut();
                    state.rows = old_rows;
                    state.order = old_order;
                    drop(state);
                    error.call(SilexError::Javascript(
                        "keyed row update was rejected".to_string(),
                    ));
                }
                Err(panic) => {
                    let mut cleanup_panic = dispose_map(&mut pending);
                    let mut current = old_rows.into_values().collect::<Vec<_>>();
                    let current_panic = dispose_rows(&mut current);
                    if cleanup_panic.is_none() {
                        cleanup_panic = current_panic;
                    }
                    let mut state = effect_state.borrow_mut();
                    state.rows.clear();
                    state.order.clear();
                    drop(state);
                    report_panic(&error, "Keyed list", panic);
                    drop(cleanup_panic);
                }
            }
        }),
    );
}

fn dispose_map<'scope, T: Clone + 'scope, K>(
    rows: &mut HashMap<K, RowController<'scope, T>>,
) -> Option<Box<dyn std::any::Any + Send>> {
    let mut values = rows.drain().map(|(_, row)| row).collect::<Vec<_>>();
    dispose_rows(&mut values)
}

fn report_panic(error: &ForErrorHandler, prefix: &str, panic: Box<dyn std::any::Any + Send>) {
    error.call(panic_error(prefix, panic));
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
