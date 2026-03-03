use crate::flow::for_loop::ForLoopSource;
use silex_core::reactivity::{
    Effect, NodeId, ReadSignal, WriteSignal, batch, create_scope, dispose, signal,
};
use silex_core::traits::{IntoRx, RxRead, RxWrite};
use silex_dom::prelude::View;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::Node;

/// Index 组件：类似于 For，但基于索引（Index）进行迭代。
///
/// 当列表顺序发生变化时，DOM 节点不会移动，只是对应的数据 Signal 会更新。
/// 适用于基础类型列表或无唯一 Key 的列表。
#[derive(Clone)]
pub struct Index<ItemsFn, Item, Items, MapFn, V> {
    items: Rc<ItemsFn>,
    map: Rc<MapFn>,
    _marker: std::marker::PhantomData<(Item, Items, V)>,
}

impl<ItemsFn, Item, Items, MapFn, V> Index<ItemsFn, Item, Items, MapFn, V>
where
    ItemsFn: RxRead<Value = Items> + 'static,
    Items: ForLoopSource<Item = Item> + 'static,
    MapFn: Fn(ReadSignal<Item>, usize) -> V + 'static,
    V: View,
    Item: 'static,
{
    pub fn new(items: impl IntoRx<Value = Items, RxType = ItemsFn>, map: MapFn) -> Self {
        Self {
            items: Rc::new(items.into_rx()),
            map: Rc::new(map),
            _marker: std::marker::PhantomData,
        }
    }
}

// Helper struct for row state
struct IndexRow<Item> {
    // setter to update the signal
    setter: WriteSignal<Item>,
    scope_id: NodeId,
    // Store nodes for removal
    nodes: Vec<Node>,
}

impl<ItemsFn, Item, Items, MapFn, V> View for Index<ItemsFn, Item, Items, MapFn, V>
where
    ItemsFn: RxRead<Value = Items> + Clone + 'static,
    Items: ForLoopSource<Item = Item> + 'static,
    MapFn: Fn(ReadSignal<Item>, usize) -> V + 'static,
    V: View,
    Item: Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_index_internal(self.items, self.map, parent, attrs);
    }

    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_index_internal(self.items.clone(), self.map.clone(), parent, attrs);
    }
}

fn mount_index_internal<ItemsFn, Item, Items, MapFn, V>(
    items_fn: Rc<ItemsFn>,
    map_fn: Rc<MapFn>,
    parent: &Node,
    _attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    ItemsFn: RxRead<Value = Items> + 'static,
    Items: ForLoopSource<Item = Item> + 'static,
    MapFn: Fn(ReadSignal<Item>, usize) -> V + 'static,
    V: View,
    Item: Clone + 'static,
{
    let document = silex_dom::document();
    let start_node: Node = document.create_comment("index-start").into();
    let _ = parent.append_child(&start_node);

    let end_node: Node = document.create_comment("index-end").into();
    let _ = parent.append_child(&end_node);

    let rows = Rc::new(RefCell::new(Vec::<IndexRow<Item>>::new()));

    Effect::new(move |_| {
        items_fn.with(|items| {
            let items_slice = match items.as_slice() {
                Ok(s) => s,
                Err(e) => {
                    silex_core::error::handle_error(e);
                    return;
                }
            };

            let mut rows_lock = rows.borrow_mut();

            batch(|| {
                let new_len = items_slice.len();
                let old_len = rows_lock.len();
                let common_len = std::cmp::min(new_len, old_len);

                for (i, item) in items_slice.iter().take(common_len).enumerate() {
                    rows_lock[i].setter.set(item.clone());
                }

                if new_len > old_len {
                    for (i, item) in items_slice[common_len..].iter().enumerate() {
                        let (set, scope_id, nodes, fragment_node) =
                            silex_core::reactivity::untrack(|| {
                                let real_index = common_len + i;
                                let (get, set) = signal(item.clone());
                                let fragment = document.create_document_fragment();
                                let fragment_node: Node = fragment.clone().into();
                                let fragment_node_clone = fragment_node.clone();
                                let map_fn = map_fn.clone();

                                let scope_id = create_scope(move || {
                                    (map_fn)(get, real_index)
                                        .mount(&fragment_node_clone, Vec::new());
                                });

                                let nodes_list = fragment.child_nodes();
                                let len = nodes_list.length();
                                let mut nodes = Vec::with_capacity(len as usize);
                                for j in 0..len {
                                    if let Some(n) = nodes_list.item(j) {
                                        nodes.push(n);
                                    }
                                }
                                (set, scope_id, nodes, fragment_node)
                            });

                        if let Some(p) = end_node.parent_node() {
                            let _ = p.insert_before(&fragment_node, Some(&end_node));
                        }

                        rows_lock.push(IndexRow {
                            setter: set,
                            scope_id,
                            nodes,
                        });
                    }
                }

                if old_len > new_len {
                    let to_remove = rows_lock.split_off(new_len);
                    for row in to_remove {
                        dispose(row.scope_id);
                        for node in row.nodes {
                            if let Some(p) = node.parent_node() {
                                let _ = p.remove_child(&node);
                            }
                        }
                    }
                }
            });
        });
    });
}
