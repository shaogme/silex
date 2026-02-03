use crate::SilexError;
use crate::flow::for_loop::IntoForLoopResult;
use silex_core::reactivity::{
    Effect, IntoSignal, NodeId, ReadSignal, WriteSignal, batch, create_scope, dispose, signal,
};
use silex_core::traits::{Get, Set};
use silex_dom::View;
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
    ItemsFn: Get<Value = Items> + 'static,
    Items: IntoForLoopResult<Item = Item>,
    MapFn: Fn(ReadSignal<Item>, usize) -> V + 'static,
    V: View,
    Item: 'static,
{
    pub fn new(items: impl IntoSignal<Value = Items, Signal = ItemsFn>, map: MapFn) -> Self {
        Self {
            items: Rc::new(items.into_signal()),
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
    ItemsFn: Get<Value = Items> + 'static,
    Items: IntoForLoopResult<Item = Item> + 'static,
    <Items as IntoForLoopResult>::Iter: IntoIterator<Item = Item>,
    MapFn: Fn(ReadSignal<Item>, usize) -> V + 'static,
    V: View,
    Item: Clone + 'static, // Item needs clone for Signal updates
{
    fn mount(self, parent: &Node) {
        let document = silex_dom::document();
        let start_marker = document.create_comment("index-start");
        let start_node: Node = start_marker.into();

        if let Err(e) = parent.append_child(&start_node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
            return;
        }

        let end_marker = document.create_comment("index-end");
        let end_node: Node = end_marker.into();

        if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
            return;
        }

        let rows = Rc::new(RefCell::new(Vec::<IndexRow<Item>>::new()));
        let items_fn = self.items;
        let map_fn = self.map;

        Effect::new(move |_| {
            let result = items_fn.get().into_result();
            let items_iter = match result {
                Ok(iter) => iter,
                Err(e) => {
                    silex_core::error::handle_error(e);
                    return;
                }
            };

            let items_vec: Vec<Item> = items_iter.into_iter().collect();
            let mut rows_lock = rows.borrow_mut();

            batch(|| {
                let new_len = items_vec.len();
                let old_len = rows_lock.len();
                let common_len = std::cmp::min(new_len, old_len);

                // 1. Update existing rows
                for (i, item) in items_vec.iter().take(common_len).enumerate() {
                    rows_lock[i].setter.set(item.clone());
                }

                // 2. Add new rows
                if new_len > old_len {
                    for (i, item) in items_vec.into_iter().skip(common_len).enumerate() {
                        let real_index = common_len + i;
                        let (get, set) = signal(item);

                        let fragment = document.create_document_fragment();
                        let fragment_node: Node = fragment.clone().into();
                        let fragment_node_clone = fragment_node.clone();
                        let map = map_fn.clone();

                        let scope_id = create_scope(move || {
                            map(get, real_index).mount(&fragment_node_clone);
                        });

                        let nodes_list = fragment.child_nodes();
                        let mut nodes = Vec::new();
                        for j in 0..nodes_list.length() {
                            if let Some(n) = nodes_list.item(j) {
                                nodes.push(n);
                            }
                        }

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

                // 3. Remove extra rows
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
    }
}
