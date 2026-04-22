use crate::{SilexError, SilexResult};
use silex_core::reactivity::{
    Effect, NodeId, ReadSignal, Signal, WriteSignal, batch, create_scope, dispose,
};
use silex_core::traits::{RxRead, RxWrite};
use silex_dom::prelude::*;
use silex_macros::component;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;
use web_sys::Node;

/// Trait to unify different types of data sources that can be used in a `For` loop
/// via zero-copy slice access.
///
/// Unlike the previous iteration approach which required cloning the collection,
/// this trait allows the `For` component to inspect the data as a slice `&[T]`.
/// We only clone the individual `Item` when we actually need to create a new row.
pub trait ForLoopSource {
    type Item: Clone;

    /// Returns a slice of the items.
    /// If the source represents an "empty" state (e.g. Option::None), return an empty slice.
    /// If the source represents an error (e.g. Result::Err), return the error.
    fn as_slice(&self) -> SilexResult<&[Self::Item]>;
}

/// Helper trait to extract Key type from function.
pub trait LoopKey<Item> {
    type Key: Hash + Eq + Clone + 'static;
    fn get_key(&self, item: &Item) -> Self::Key;
}

impl<F, Item, K> LoopKey<Item> for F
where
    F: Fn(&Item) -> K,
    K: Hash + Eq + Clone + 'static,
{
    type Key = K;

    fn get_key(&self, item: &Item) -> Self::Key {
        (self)(item)
    }
}

/// Helper trait to extract View type from Map function.
pub trait LoopMap<Item> {
    type View: View;
    fn map(&self, item: Item) -> Self::View;
}

impl<F, Item, V> LoopMap<Item> for F
where
    F: Fn(Item) -> V,
    V: View + 'static,
{
    type View = V;

    fn map(&self, item: Item) -> Self::View {
        (self)(item)
    }
}

// Impl for Vec<T>
impl<T: Clone> ForLoopSource for Vec<T> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self.as_slice())
    }
}

// Impl for Option<Vec<T>>
impl<T: Clone> ForLoopSource for Option<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Some(v) => Ok(v.as_slice()),
            None => Ok(&[]),
        }
    }
}

// Impl for SilexResult<Vec<T>>
impl<T: Clone> ForLoopSource for SilexResult<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Ok(v) => Ok(v.as_slice()),
            Err(e) => Err(e.clone()),
        }
    }
}

/// 标准 component 化的 For 组件。
///
/// 使用方式：
/// ```rust,ignore
/// For(list, |item| item.id)
///     .children(|item, idx| li(format!("{}: {}", idx.get(), item.name)))
///     .error(|err| log_error(err))
/// ```
#[component(standalone = 2)]
pub fn For<ItemsFn, IS, Item, Key, MF, V>(
    items: ItemsFn,
    key: fn(&Item) -> Key,
    #[prop(render)] children: MF,
    #[prop(default = ::silex_core::error::handle_error, into)] error: ForErrorHandler,
) -> impl View
where
    ItemsFn: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = Item> + Sized + 'static,
    Item: Clone + 'static,
    Key: Hash + Eq + Clone + 'static,
    MF: Fn(ReadSignal<Item>, ReadSignal<usize>) -> V + Clone + 'static,
    V: View + 'static,
{
    let children = children.into_owned();
    let children = Rc::new(move |item: ReadSignal<Item>, index: ReadSignal<usize>| {
        children(item, index).into_any()
    });

    ForView {
        items,
        key: *key,
        children,
        error,
        _marker: PhantomData,
    }
}

#[derive(Clone)]
pub struct ForErrorHandler(Rc<dyn Fn(SilexError)>);

impl ForErrorHandler {
    pub fn call(&self, err: SilexError) {
        (self.0)(err);
    }
}

impl<F> From<F> for ForErrorHandler
where
    F: Fn(SilexError) + 'static,
{
    fn from(value: F) -> Self {
        Self(Rc::new(value))
    }
}

#[derive(Clone)]
struct ForView<'a, ItemsFn, IS, Item, Key> {
    items: Prop<'a, ItemsFn>,
    key: fn(&Item) -> Key,
    children: Rc<dyn Fn(ReadSignal<Item>, ReadSignal<usize>) -> AnyView + 'static>,
    error: Prop<'a, ForErrorHandler>,
    _marker: PhantomData<(IS, Item, Key)>,
}

impl<'a, ItemsFn, IS, Item, Key> ApplyAttributes for ForView<'a, ItemsFn, IS, Item, Key> {}

impl<'a, ItemsFn, IS, Item, Key> View for ForView<'a, ItemsFn, IS, Item, Key>
where
    ItemsFn: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = Item> + Sized + 'static,
    Key: Hash + Eq + Clone + 'static,
    Item: Clone + 'static,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_for_internal(
            Prop::new_owned(self.items.clone()),
            self.key,
            self.children.clone(),
            Prop::new_owned(self.error.clone()),
            parent,
            attrs,
        );
    }
}

struct ForRow<Item> {
    item_setter: WriteSignal<Item>,
    index_setter: WriteSignal<usize>,
    scope_id: NodeId,
    nodes: Vec<Node>,
}

fn mount_for_internal<'a, ItemsFn, IS, Item, Key>(
    items_fn: Prop<'a, ItemsFn>,
    key_fn: fn(&Item) -> Key,
    children_fn: Rc<dyn Fn(ReadSignal<Item>, ReadSignal<usize>) -> AnyView + 'static>,
    error: Prop<'a, ForErrorHandler>,
    parent: &Node,
    _attrs: Vec<PendingAttribute>,
) where
    ItemsFn: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = Item> + Sized + 'static,
    Key: Hash + Eq + Clone + 'static,
    Item: Clone + 'static,
{
    let items_fn = items_fn.into_owned();
    let error = error.into_owned();
    let document = silex_dom::document();

    // 1. Create Anchors
    let start_marker = document.create_comment("for-start");
    let start_node: Node = start_marker.into();

    if let Err(e) = parent.append_child(&start_node).map_err(SilexError::from) {
        silex_core::error::handle_error(e);
        return;
    }

    let end_marker = document.create_comment("for-end");
    let end_node: Node = end_marker.into();

    if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
        silex_core::error::handle_error(e);
        return;
    }

    let active_rows = Rc::new(RefCell::new(HashMap::<Key, ForRow<Item>>::new()));

    Effect::new(move |_| {
        // Zero-Copy Optimization:
        // We use `with` to access the `Items` by reference.
        // `as_slice()` gives us `&[Item]` without cloning the collection.
        items_fn.with(|items| {
            let items_slice = match items.as_slice() {
                Ok(s) => s,
                Err(e) => {
                    error.call(e);
                    return;
                }
            };

            batch(|| {
                let rows_snapshot = active_rows.borrow();
                let mut new_keys = HashSet::new();
                // (Key, Nodes, ScopeId, Optional Fragment for initial insert)
                let mut new_rows_order = Vec::with_capacity(items_slice.len());
                let mut new_rows_to_insert = Vec::with_capacity(items_slice.len());

                for (index, item_ref) in items_slice.iter().enumerate() {
                    let key = key_fn(item_ref);
                    if !new_keys.insert(key.clone()) {
                        error.call(SilexError::Javascript(
                            "Duplicate key detected in For loop; each key must be unique"
                                .to_string(),
                        ));
                        continue;
                    }

                    if let Some(row) = rows_snapshot.get(&key) {
                        row.item_setter.set(item_ref.clone());
                        row.index_setter.set(index);
                        new_rows_order.push((key, row.nodes.clone(), row.scope_id, None));
                    } else {
                        let (nodes, scope_id, fragment, item_setter, index_setter) =
                            silex_core::reactivity::untrack(|| {
                                let fragment = document.create_document_fragment();
                                let fragment_node: Node = fragment.clone().into();
                                let children_fn = children_fn.clone();
                                let (item_get, item_setter) = Signal::pair(item_ref.clone());
                                let (index_get, index_setter) = Signal::pair(index);

                                let scope_id = create_scope(move || {
                                    let view = (children_fn.as_ref())(item_get, index_get);
                                    view.mount(&fragment_node, Vec::new());
                                });

                                let nodes_list = fragment.child_nodes();
                                let len = nodes_list.length();
                                let mut nodes = Vec::with_capacity(len as usize);
                                for i in 0..len {
                                    if let Some(n) = nodes_list.item(i) {
                                        nodes.push(n);
                                    }
                                }
                                (nodes, scope_id, fragment, item_setter, index_setter)
                            });

                        new_rows_to_insert.push((
                            key.clone(),
                            item_setter,
                            index_setter,
                            scope_id,
                            nodes.clone(),
                        ));
                        new_rows_order.push((key, nodes, scope_id, Some(fragment)));
                    };
                }

                drop(rows_snapshot);
                let mut rows_map = active_rows.borrow_mut();

                for (key, item_setter, index_setter, scope_id, nodes) in new_rows_to_insert {
                    rows_map.insert(
                        key,
                        ForRow {
                            item_setter,
                            index_setter,
                            scope_id,
                            nodes,
                        },
                    );
                }

                // Cleanup removed rows
                rows_map.retain(|k, row| {
                    if !new_keys.contains(k) {
                        for node in &row.nodes {
                            if let Some(p) = node.parent_node() {
                                let _ = p.remove_child(node);
                            }
                        }
                        dispose(row.scope_id);
                        false
                    } else {
                        true
                    }
                });

                // Reorder / Insert
                let mut cursor = start_node.next_sibling();

                for (_key, nodes, _id, fragment_opt) in new_rows_order {
                    if let Some(frag) = fragment_opt {
                        let effective_cursor = cursor.as_ref().unwrap_or(&end_node);

                        if let Some(parent) = effective_cursor.parent_node() {
                            let _ = parent.insert_before(&frag, Some(effective_cursor));
                        }
                    } else {
                        if nodes.is_empty() {
                            continue;
                        }

                        let first_node = &nodes[0];

                        let is_in_place = if let Some(ref c) = cursor {
                            c.is_same_node(Some(first_node))
                        } else {
                            false
                        };

                        if is_in_place {
                            for _ in 0..nodes.len() {
                                cursor = cursor.and_then(|c| c.next_sibling());
                            }
                        } else {
                            let effective_cursor = cursor.as_ref().unwrap_or(&end_node);
                            if let Some(parent) = effective_cursor.parent_node() {
                                for node in &nodes {
                                    let _ = parent.insert_before(node, Some(effective_cursor));
                                }
                            }
                        }
                    }
                }
            });
        });
    });
}
