use crate::dom::View;
use crate::reactivity::{NodeId, create_effect, create_scope, dispose};
use crate::{SilexError, SilexResult};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use web_sys::Node;

/// Trait to unify different types of data sources that can be used in a `For` loop.
/// This allows `step` to return `Vec<T>`, `Result<Vec<T>>`, etc.
pub trait IntoForLoopResult {
    type Item;
    type Iter: IntoIterator<Item = Self::Item>;

    fn into_result(self) -> SilexResult<Self::Iter>;
}

// Impl for Vec<T>
impl<T> IntoForLoopResult for Vec<T> {
    type Item = T;
    type Iter = std::vec::IntoIter<T>;

    fn into_result(self) -> SilexResult<Self::Iter> {
        Ok(self.into_iter())
    }
}

// Impl for Option<Vec<T>>
impl<T> IntoForLoopResult for Option<Vec<T>> {
    type Item = T;
    type Iter = std::vec::IntoIter<T>;

    fn into_result(self) -> SilexResult<Self::Iter> {
        Ok(self.unwrap_or_default().into_iter())
    }
}

// Impl for SilexResult<Vec<T>>
impl<T> IntoForLoopResult for SilexResult<Vec<T>> {
    type Item = T;
    type Iter = std::vec::IntoIter<T>;

    fn into_result(self) -> SilexResult<Self::Iter> {
        self.map(|v| v.into_iter())
    }
}

// Ensure SilexResult is available if it's generic, but here we cover `SilexResult<Vec<T>>` explicitly.
// We could add more impls as needed (e.g. for &[T] or other collections), but Vec is 99% of use cases.

pub struct For<ItemsFn, Item, Items, KeyFn, Key, MapFn, V> {
    items: Rc<ItemsFn>,
    key: Rc<KeyFn>,
    map: Rc<MapFn>,
    _marker: std::marker::PhantomData<(Item, Items, Key, V)>,
}

impl<ItemsFn, Item, Items, KeyFn, Key, MapFn, V> For<ItemsFn, Item, Items, KeyFn, Key, MapFn, V>
where
    // ItemsFn returns the Source directly (e.g. Vec or Result<Vec>)
    ItemsFn: Fn() -> Items + 'static,
    Items: IntoForLoopResult<Item = Item>,
    KeyFn: Fn(&Item) -> Key + 'static,
    MapFn: Fn(Item) -> V + 'static,
    V: View,
    Item: 'static,
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
    ItemsFn: Fn() -> Items + 'static,
    Items: IntoForLoopResult<Item = Item> + 'static,
    // We need the Iterator produced by the result to be iterable
    <Items as IntoForLoopResult>::Iter: IntoIterator<Item = Item>,
    KeyFn: Fn(&Item) -> Key + 'static,
    Key: std::hash::Hash + Eq + Clone + 'static,
    MapFn: Fn(Item) -> V + 'static,
    V: View,
    Item: 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::dom::document();

        // 1. Create Anchors
        let start_marker = document.create_comment("for-start");
        let start_node: Node = start_marker.into();

        if let Err(e) = parent.append_child(&start_node).map_err(SilexError::from) {
            crate::error::handle_error(e);
            return;
        }

        let end_marker = document.create_comment("for-end");
        let end_node: Node = end_marker.into();

        if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
            crate::error::handle_error(e);
            return;
        }

        let items_fn = self.items;
        let key_fn = self.key;
        let map_fn = self.map;

        // Store: (Nodes, ScopeId)
        let active_rows = Rc::new(RefCell::new(HashMap::<Key, (Vec<Node>, NodeId)>::new()));

        create_effect(move || {
            let mut rows_map = active_rows.borrow_mut();

            // Use the trait to convert whatever Items is into SilexResult<Iterator>
            let result = (items_fn)().into_result();

            let items_iter = match result {
                Ok(iter) => iter,
                Err(e) => {
                    crate::error::handle_error(e);
                    return;
                }
            };

            let mut new_keys = HashSet::new();
            // (Key, Nodes, ScopeId, Optional Fragment for initial insert)
            let mut new_rows_order = Vec::new();

            for item in items_iter {
                let key = (key_fn)(&item);
                new_keys.insert(key.clone());

                if let Some((nodes, id)) = rows_map.get(&key) {
                    // Existing row
                    new_rows_order.push((key, nodes.clone(), *id, None));
                } else {
                    // New row
                    let fragment = document.create_document_fragment();
                    let fragment_node: Node = fragment.clone().into();

                    let map_fn = map_fn.clone();

                    let scope_id = create_scope(move || {
                        let view = (map_fn)(item);
                        view.mount(&fragment_node);
                    });

                    // Collect nodes from fragment before they are moved (they are not moved yet)
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
                    // Insert fragment before cursor (or end_marker if cursor is None, but logic ensures cursor covers range)
                    // If cursor is None, it means we are at the end of the list (past the last node? No, end_node is there).
                    // BUT: cursor might be the end_node.

                    let effective_cursor = cursor.as_ref().unwrap_or(&end_node);

                    if let Some(parent) = effective_cursor.parent_node() {
                        let _ = parent.insert_before(&frag, Some(effective_cursor));
                    }
                    // Inserted nodes are now in DOM. Updates rows_map.
                    rows_map.insert(key, (nodes, id));
                } else {
                    // Existing row. Check if in place.
                    // We check if the first node of this row matches `cursor`.
                    // If row has 0 nodes, it's a no-op (display-wise).
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
    }
}
