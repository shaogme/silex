use super::owner::{DomRange, RowController, RowRender, RowRenderArgs};
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

/// Keyed list with persistent row controllers and stable keyed ranges.
pub struct KeyedLoopView<'scope, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope> + 'scope>,
    pub error: ForErrorHandler,
    pub _marker: std::marker::PhantomData<(IS, T)>,
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
            self.view_fn.clone(),
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
            self.view_fn,
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
            self.view_fn.clone(),
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
        mount_indexed_list(owner, parent, self.each, self.view_fn, attrs);
    }
}

fn mount_indexed_list<'scope, IF, IS, T, F>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    source: IF,
    view_fn: Rc<F>,
    attrs: Vec<PendingAttribute<'scope>>,
) where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    F: Fn(T, usize) -> AnyView<'scope> + 'scope + ?Sized,
{
    let inputs = runtime_inputs_of(source.clone());
    if let Err(error) = owner.validate_inputs(&inputs) {
        silex_core::error::handle_error(error);
        return;
    }
    let token = owner.token();
    let range = match DomRange::append(parent, "for") {
        Ok(range) => range,
        Err(error) => {
            silex_core::error::handle_error(error);
            return;
        }
    };
    let render = RowRender::new(move |args: RowRenderArgs<'scope, T>| {
        let RowRenderArgs {
            item,
            index,
            parent,
            attrs,
            owner: token,
        } = args;
        view_fn(item, index).mount_owned(&token, &parent, attrs);
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
            let snapshot = source.with(|items| items.as_slice().map(|values| values.to_vec()));
            match snapshot {
                Ok(values) => {
                    let mut rows = effect_rows.borrow_mut();
                    while rows.len() > values.len() {
                        let mut row = rows.pop().expect("row length checked before pop");
                        row.dispose();
                    }
                    for (index, item) in values.into_iter().enumerate() {
                        if let Some(row) = rows.get_mut(index) {
                            row.update(item, index);
                            continue;
                        }
                        let Ok(row_range) = DomRange::before(&end, "for-row") else {
                            continue;
                        };
                        rows.push(RowController::new(
                            &token,
                            row_range,
                            render.clone(),
                            RuntimeInputs::new(),
                            attrs.clone(),
                            item,
                            index,
                        ));
                    }
                }
                Err(error) => {
                    let mut rows = mem::take(&mut *effect_rows.borrow_mut());
                    let panic = dispose_rows(&mut rows);
                    if let Some(panic) = panic {
                        resume_unwind(panic);
                    }
                    silex_core::error::handle_error(error);
                }
            }
        }),
    );
}

struct KeyedRows<'scope, T, K> {
    rows: HashMap<K, RowController<'scope, T>>,
    order: Vec<K>,
}

fn mount_keyed_list<'scope, IF, IS, T, K, F>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    source: IF,
    key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    view_fn: Rc<F>,
    error: ForErrorHandler,
    attrs: Vec<PendingAttribute<'scope>>,
) where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    K: std::hash::Hash + Eq + Clone + 'scope,
    F: Fn(T, usize) -> AnyView<'scope> + 'scope + ?Sized,
{
    let inputs = runtime_inputs_of(source.clone());
    if let Err(error) = owner.validate_inputs(&inputs) {
        silex_core::error::handle_error(error);
        return;
    }
    let token = owner.token();
    let range = match DomRange::append(parent, "for") {
        Ok(range) => range,
        Err(error) => {
            silex_core::error::handle_error(error);
            return;
        }
    };
    let render = RowRender::new(move |args: RowRenderArgs<'scope, T>| {
        let RowRenderArgs {
            item,
            index,
            parent,
            attrs,
            owner: token,
        } = args;
        view_fn(item, index).mount_owned(&token, &parent, attrs);
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
            let snapshot = source.with(|items| items.as_slice().map(|values| values.to_vec()));
            let values = match snapshot {
                Ok(values) => values,
                Err(source_error) => {
                    error.call(source_error);
                    return;
                }
            };

            let mut keys = Vec::with_capacity(values.len());
            let mut seen = HashSet::with_capacity(values.len());
            for item in &values {
                let key = key_fn(item);
                if !seen.insert(key.clone()) {
                    error.call(SilexError::Reactivity(
                        "duplicate key in keyed list".to_string(),
                    ));
                    return;
                }
                keys.push(key);
            }

            let mut state = effect_state.borrow_mut();
            let mut old_rows = mem::take(&mut state.rows);
            let old_order = mem::take(&mut state.order);
            let mut next_rows = HashMap::with_capacity(keys.len());
            let mut next_order = Vec::with_capacity(keys.len());

            for (index, (key, item)) in keys.iter().cloned().zip(values).enumerate() {
                if let Some(mut row) = old_rows.remove(&key) {
                    row.update(item, index);
                    next_order.push(key.clone());
                    next_rows.insert(key, row);
                    continue;
                }
                let Ok(row_range) = DomRange::before(&end, "for-row") else {
                    continue;
                };
                next_order.push(key.clone());
                next_rows.insert(
                    key,
                    RowController::new(
                        &token,
                        row_range,
                        render.clone(),
                        RuntimeInputs::new(),
                        attrs.clone(),
                        item,
                        index,
                    ),
                );
            }

            let mut removed = Vec::with_capacity(old_rows.len());
            for key in old_order {
                if let Some(row) = old_rows.remove(&key) {
                    removed.push(row);
                }
            }
            let panic = dispose_rows(&mut removed);
            for key in &next_order {
                if let Some(row) = next_rows.get(key) {
                    row.move_before(&end);
                }
            }
            state.rows = next_rows;
            state.order = next_order;
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
        }),
    );
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
