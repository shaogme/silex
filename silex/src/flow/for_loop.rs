use crate::{SilexError, SilexResult};
use silex_core::reactivity::{Effect, NodeId, batch, create_scope, dispose};
use silex_core::traits::With;
use silex_dom::View;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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

pub struct For<ItemsFn, Item, Items, KeyFn, Key, MapFn, V> {
    items: Rc<ItemsFn>,
    key: Rc<KeyFn>,
    map: Rc<MapFn>,
    _marker: std::marker::PhantomData<(Item, Items, Key, V)>,
}

impl<ItemsFn, Item, Items, KeyFn, Key, MapFn, V> Clone
    for For<ItemsFn, Item, Items, KeyFn, Key, MapFn, V>
{
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            key: self.key.clone(),
            map: self.map.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<ItemsFn, Item, Items, KeyFn, Key, MapFn, V> For<ItemsFn, Item, Items, KeyFn, Key, MapFn, V>
where
    // ItemsFn returns the Source directly (e.g. Vec or Result<Vec>)
    // We access it by reference via `With`.
    ItemsFn: With<Value = Items> + 'static,
    Items: ForLoopSource<Item = Item> + 'static,
    KeyFn: Fn(&Item) -> Key + 'static,
    MapFn: Fn(Item) -> V + 'static,
    V: View,
    Item: Clone + 'static,
{
    pub fn new(items: ItemsFn, key: KeyFn, map: MapFn) -> Self {
        Self {
            items: Rc::new(items),
            key: Rc::new(key),
            map: Rc::new(map),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<ItemsFn, Item, Items, KeyFn, Key, MapFn, V> View
    for For<ItemsFn, Item, Items, KeyFn, Key, MapFn, V>
where
    ItemsFn: With<Value = Items> + 'static,
    Items: ForLoopSource<Item = Item> + 'static,
    KeyFn: Fn(&Item) -> Key + 'static,
    Key: std::hash::Hash + Eq + Clone + 'static,
    MapFn: Fn(Item) -> V + 'static,
    V: View,
    Item: Clone + 'static,
{
    fn mount(self, parent: &Node) {
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

        let items_fn = self.items;
        let key_fn = self.key;
        let map_fn = self.map;

        // Store: (Nodes, ScopeId)
        let active_rows = Rc::new(RefCell::new(HashMap::<Key, (Vec<Node>, NodeId)>::new()));

        Effect::new(move |_| {
            let mut rows_map = active_rows.borrow_mut();

            // Zero-Copy Optimization:
            // We use `with` to access the `Items` by reference.
            // `as_slice()` gives us `&[Item]` without cloning the collection.
            items_fn.with(|items| {
                let items_slice = match items.as_slice() {
                    Ok(s) => s,
                    Err(e) => {
                        silex_core::error::handle_error(e);
                        return;
                    }
                };

                batch(|| {
                    let mut new_keys = HashSet::new();
                    // (Key, Nodes, ScopeId, Optional Fragment for initial insert)
                    let mut new_rows_order = Vec::with_capacity(items_slice.len());

                    for item_ref in items_slice {
                        // Calculate key from reference
                        let key = (key_fn)(item_ref);
                        new_keys.insert(key.clone());

                        if let Some((nodes, id)) = rows_map.get(&key) {
                            // Existing row: reuse nodes and scope
                            new_rows_order.push((key, nodes.clone(), *id, None));
                        } else {
                            // New row: We MUST clone the Item here to pass ownership to map_fn.
                            // This is the only place we clone individual items, and only for new rows.
                            let item_owned = item_ref.clone();

                            let fragment = document.create_document_fragment();
                            let fragment_node: Node = fragment.clone().into();

                            let map_fn = map_fn.clone();

                            let scope_id = create_scope(move || {
                                let view = (map_fn)(item_owned);
                                view.mount(&fragment_node);
                            });

                            // Collect nodes from fragment before they are moved
                            let nodes_list = fragment.child_nodes();
                            let len = nodes_list.length();
                            let mut nodes = Vec::with_capacity(len as usize);
                            for i in 0..len {
                                if let Some(n) = nodes_list.item(i) {
                                    nodes.push(n);
                                }
                            }

                            new_rows_order.push((key, nodes, scope_id, Some(fragment)));
                        };
                    }

                    // Cleanup removed rows
                    rows_map.retain(|k, (nodes, id)| {
                        if !new_keys.contains(k) {
                            // Remove all nodes for this row
                            for node in nodes {
                                if let Some(p) = node.parent_node() {
                                    let _ = p.remove_child(node);
                                }
                            }
                            dispose(*id);
                            false
                        } else {
                            true
                        }
                    });

                    // Reorder / Insert
                    // Start scanning from start_marker
                    let mut cursor = start_node.next_sibling();

                    for (key, nodes, id, fragment_opt) in new_rows_order {
                        // If this is a new row with a fragment, insert it efficiently
                        if let Some(frag) = fragment_opt {
                            let effective_cursor = cursor.as_ref().unwrap_or(&end_node);

                            if let Some(parent) = effective_cursor.parent_node() {
                                let _ = parent.insert_before(&frag, Some(effective_cursor));
                            }
                            // Inserted nodes are now in DOM. Update rows_map.
                            rows_map.insert(key, (nodes, id));
                        } else {
                            // Existing row. Check if in place.
                            if nodes.is_empty() {
                                rows_map.insert(key, (nodes, id));
                                continue;
                            }

                            let first_node = &nodes[0];

                            // Check if first_node is at cursor
                            let is_in_place = if let Some(ref c) = cursor {
                                c.is_same_node(Some(first_node))
                            } else {
                                false
                            };

                            if is_in_place {
                                // It matches. This row is correct.
                                // Advance cursor past this row's nodes.
                                for _ in 0..nodes.len() {
                                    cursor = cursor.and_then(|c| c.next_sibling());
                                }
                            } else {
                                // Not in place. Move nodes.
                                let effective_cursor = cursor.as_ref().unwrap_or(&end_node);
                                if let Some(parent) = effective_cursor.parent_node() {
                                    for node in &nodes {
                                        let _ = parent.insert_before(node, Some(effective_cursor));
                                    }
                                }
                                // After moving, they are before cursor. Cursor stays same.
                            }
                            rows_map.insert(key, (nodes, id));
                        }
                    }
                });
            });
        });
    }
}
