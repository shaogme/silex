use crate::flow::for_loop::ForLoopSource;
use silex_core::reactivity::{
    Effect, NodeId, ReadSignal, Signal, WriteSignal, batch, create_scope, dispose,
};
use silex_core::traits::{RxRead, RxWrite};
use silex_dom::prelude::*;
use silex_macros::component;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::Node;

/// Index 组件：类似于 For，但基于索引（Index）进行迭代。
///
/// 当列表顺序发生变化时，DOM 节点不会移动，只是对应的数据 Signal 会更新。
/// 适用于基础类型列表或无唯一 Key 的列表。
///
/// 使用方式：
/// ```rust
/// Index(list).children(|item, index| li(item))
/// ```
#[component]
pub fn Index<IF, I, IS, MF, V>(
    each: IF,
    children: MF,
) -> impl Mount + MountRef
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = I> + 'static,
    MF: Fn(ReadSignal<I>, usize) -> V + Clone + 'static,
    V: Mount + 'static,
    I: Clone + 'static,
{
    IndexView {
        each: each.clone(),
        children: children.clone(),
        _marker: std::marker::PhantomData,
    }
}

struct IndexView<IF, MF, I, IS, V> {
    each: IF,
    children: MF,
    _marker: std::marker::PhantomData<(I, IS, V)>,
}

impl<IF, MF, I, IS, V> Clone for IndexView<IF, MF, I, IS, V>
where
    IF: Clone,
    MF: Clone,
{
    fn clone(&self) -> Self {
        Self {
            each: self.each.clone(),
            children: self.children.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

// Helper struct for row state
struct IndexRow<Item> {
    setter: WriteSignal<Item>,
    scope_id: NodeId,
    nodes: Vec<Node>,
}

impl<IF, MF, I, IS, V> ApplyAttributes for IndexView<IF, MF, I, IS, V> {}

impl<IF, MF, I, IS, V> Mount for IndexView<IF, MF, I, IS, V>
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = I> + 'static,
    MF: Fn(ReadSignal<I>, usize) -> V + Clone + 'static,
    V: Mount + 'static,
    I: Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_index_logic(self.each, self.children, parent, attrs);
    }
}

impl<IF, MF, I, IS, V> AutoReactiveView for IndexView<IF, MF, I, IS, V>
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = I> + 'static,
    MF: Fn(ReadSignal<I>, usize) -> V + Clone + 'static,
    V: Mount + 'static,
    I: Clone + 'static,
{
}

impl<IF, MF, I, IS, V> MountRef for IndexView<IF, MF, I, IS, V>
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = I> + 'static,
    MF: Fn(ReadSignal<I>, usize) -> V + Clone + 'static,
    V: Mount + 'static,
    I: Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_index_logic(self.each.clone(), self.children.clone(), parent, attrs);
    }
}

fn mount_index_logic<IF, MF, I, IS, V>(
    items_fn: IF,
    map_fn: MF,
    parent: &Node,
    _attrs: Vec<PendingAttribute>,
) where
    IF: RxRead<Value = IS> + 'static,
    IS: ForLoopSource<Item = I> + 'static,
    MF: Fn(ReadSignal<I>, usize) -> V + 'static,
    V: Mount,
    I: Clone + 'static,
{
    let document = silex_dom::document();
    let start_node: Node = document.create_comment("index-start").into();
    let _ = parent.append_child(&start_node);

    let end_node: Node = document.create_comment("index-end").into();
    let _ = parent.append_child(&end_node);

    let rows = Rc::new(RefCell::new(Vec::<IndexRow<I>>::new()));
    let map_fn = Rc::new(map_fn);

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
                                let (get, set) = Signal::pair(item.clone());
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
