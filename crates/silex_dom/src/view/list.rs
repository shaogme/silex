use crate::attribute::PendingAttribute;
use crate::view::{AnyView, ApplyAttributes, View, ViewOwner};
use silex_core::SilexError;
use silex_core::traits::{ForErrorHandler, ForLoopSource, RxRead};
use std::hash::Hash;
use std::rc::Rc;
use web_sys::Node;

/// List view that rebuilds rows under one owner on every source update.
/// Keyed reuse is deliberately deferred until a row controller owns its child
/// scope; no copied scope id is retained here.
pub struct KeyedLoopView<'scope, 'run, IF, IS, T, K> {
    pub each: IF,
    pub key_fn: Rc<dyn Fn(&T) -> K + 'scope>,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope, 'run> + 'scope>,
    pub error: ForErrorHandler,
    pub _marker: std::marker::PhantomData<(IS, T)>,
}

impl<'scope, 'run, IF, IS, T, K> ApplyAttributes<'scope, 'run>
    for KeyedLoopView<'scope, 'run, IF, IS, T, K>
{
}

impl<'scope, 'run, IF, IS, T, K> View<'scope, 'run> for KeyedLoopView<'scope, 'run, IF, IS, T, K>
where
    IF: RxRead<Value = IS> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + Sized + 'scope,
    K: Hash + Eq + Clone + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_list(
            owner,
            parent,
            self.each.clone(),
            self.view_fn.clone(),
            self.error.clone(),
            attrs,
        );
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        mount_list(owner, parent, self.each, self.view_fn, self.error, attrs);
    }
}

pub struct IndexedLoopView<'scope, 'run, IF, T, IS> {
    pub each: IF,
    pub view_fn: Rc<dyn Fn(T, usize) -> AnyView<'scope, 'run> + 'scope>,
    pub _marker: std::marker::PhantomData<(T, IS)>,
}

impl<'scope, 'run, IF, T, IS> ApplyAttributes<'scope, 'run>
    for IndexedLoopView<'scope, 'run, IF, T, IS>
{
}

impl<'scope, 'run, IF, T, IS> View<'scope, 'run> for IndexedLoopView<'scope, 'run, IF, T, IS>
where
    IF: RxRead<Value = IS> + Clone + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_list(
            owner,
            parent,
            self.each.clone(),
            self.view_fn.clone(),
            ForErrorHandler::default(),
            attrs,
        );
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        mount_list(
            owner,
            parent,
            self.each,
            self.view_fn,
            ForErrorHandler::default(),
            attrs,
        );
    }
}

fn mount_list<'scope, 'run, IF, IS, T, F>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    source: IF,
    view_fn: Rc<F>,
    error: ForErrorHandler,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
) where
    IF: RxRead<Value = IS> + 'scope,
    IS: ForLoopSource<Item = T> + 'scope,
    T: Clone + 'scope,
    F: Fn(T, usize) -> AnyView<'scope, 'run> + 'scope + ?Sized,
{
    let document = crate::document();
    let start: Node = document.create_comment("for-start").into();
    let end: Node = document.create_comment("for-end").into();
    if let Err(error) = parent.append_child(&start).map_err(SilexError::from) {
        silex_core::error::handle_error(error);
        return;
    }
    if let Err(error) = parent.append_child(&end).map_err(SilexError::from) {
        silex_core::error::handle_error(error);
        return;
    }

    let token = owner.token();
    let cleanup_start = start.clone();
    let cleanup_end = end.clone();
    owner.on_cleanup(Box::new(move || {
        clear_between(&cleanup_start, &cleanup_end);
        if let Some(parent) = cleanup_start.parent_node() {
            let _ = parent.remove_child(&cleanup_start);
        }
        if let Some(parent) = cleanup_end.parent_node() {
            let _ = parent.remove_child(&cleanup_end);
        }
    }));

    owner.effect(Box::new(move || {
        clear_between(&start, &end);
        source.with(|items| match items.as_slice() {
            Ok(values) => {
                for (index, item) in values.iter().cloned().enumerate() {
                    let view = view_fn(item, index);
                    view.mount_owned(&token, &end, attrs.clone());
                }
            }
            Err(error_value) => error.call(error_value),
        });
    }));
}

fn clear_between(start: &Node, end: &Node) {
    if let Some(parent) = start.parent_node() {
        while let Some(node) = start.next_sibling() {
            if node == *end {
                break;
            }
            let _ = parent.remove_child(&node);
        }
    }
}
