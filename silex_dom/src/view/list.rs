use crate::attribute::PendingAttribute;
use crate::view::{AnyView, ApplyAttributes, View};
use silex_core::SilexError;
use silex_core::reactivity::{
    Effect, NodeId, ReadSignal, Signal, WriteSignal, batch, create_scope, dispose, untrack,
};
use silex_core::traits::{ForErrorHandler, ForLoopSource, RxRead, RxWrite};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;
use web_sys::Node;

/// 一个特殊的视图，它通过 Key 来协调列表项。
pub struct KeyedLoopView<IF, IS, T, K> {
    pub each: IF,
    pub key_fn: fn(&T) -> K,
    pub view_fn: Rc<dyn Fn(ReadSignal<T>, ReadSignal<usize>) -> AnyView + 'static>,
    pub error: ForErrorHandler,
    pub _marker: std::marker::PhantomData<(IS, T)>,
}

impl<IF, IS, T, K> ApplyAttributes for KeyedLoopView<IF, IS, T, K> {}

impl<IF, IS, T, K> View for KeyedLoopView<IF, IS, T, K>
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = T> + Sized + 'static,
    K: Hash + Eq + Clone + 'static,
    T: Clone + 'static,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_keyed_loop_logic(
            self.each.clone(),
            self.key_fn,
            self.view_fn.clone(),
            self.error.clone(),
            parent,
            attrs,
        );
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        mount_keyed_loop_logic(
            self.each,
            self.key_fn,
            self.view_fn,
            self.error,
            parent,
            attrs,
        );
    }
}

struct KeyedLoopRow<T> {
    item_setter: WriteSignal<T>,
    index_setter: WriteSignal<usize>,
    scope_id: NodeId,
    nodes: Vec<Node>,
}

fn mount_keyed_loop_logic<IF, IS, T, K>(
    items_fn: IF,
    key_fn: fn(&T) -> K,
    view_fn: Rc<dyn Fn(ReadSignal<T>, ReadSignal<usize>) -> AnyView + 'static>,
    error: ForErrorHandler,
    parent: &Node,
    _attrs: Vec<PendingAttribute>,
) where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = T> + Sized + 'static,
    K: Hash + Eq + Clone + 'static,
    T: Clone + 'static,
{
    let document = crate::document();

    // 1. 创建锚点，保持与 view.rs 中 mount_dynamic_view_universal 一致的风格
    let start_node: Node = document.create_comment("keyed-for-start").into();
    let end_node: Node = document.create_comment("keyed-for-end").into();

    if let Err(e) = parent.append_child(&start_node).map_err(SilexError::from) {
        silex_core::error::handle_error(e);
        return;
    }
    if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
        silex_core::error::handle_error(e);
        return;
    }

    let active_rows = Rc::new(RefCell::new(HashMap::<K, KeyedLoopRow<T>>::new()));

    Effect::new(move |_| {
        items_fn.with(|items| {
            let Ok(items_slice) = items.as_slice() else {
                if let Err(e) = items.as_slice() {
                    error.call(e);
                }
                return;
            };

            batch(|| {
                let rows_snapshot = active_rows.borrow();
                let mut new_keys = HashSet::with_capacity(items_slice.len());
                let mut new_rows_order = Vec::with_capacity(items_slice.len());
                let mut new_rows_to_insert = Vec::new();

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
                        // 新项逻辑
                        let (nodes, scope_id, fragment, item_setter, index_setter) =
                            untrack(|| {
                                let fragment = document.create_document_fragment();
                                let fragment_node: Node = fragment.clone().into();
                                let (item_get, item_setter) = Signal::pair(item_ref.clone());
                                let (index_get, index_setter) = Signal::pair(index);

                                let view_fn = view_fn.clone();
                                let scope_id = create_scope(move || {
                                    let view = (view_fn)(item_get, index_get);
                                    view.mount_owned(&fragment_node, Vec::new());
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

                // 更新 active_rows 并清理多余项
                {
                    let mut rows_map = active_rows.borrow_mut();

                    // 插入新项
                    for (key, item_setter, index_setter, scope_id, nodes) in new_rows_to_insert {
                        rows_map.insert(
                            key,
                            KeyedLoopRow {
                                item_setter,
                                index_setter,
                                scope_id,
                                nodes,
                            },
                        );
                    }

                    // 清理旧项
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
                }

                // 物理协调 DOM 顺序
                let mut cursor = start_node.next_sibling();

                for (_key, nodes, _id, fragment_opt) in new_rows_order {
                    if let Some(frag) = fragment_opt {
                        // 如果是新创建的 Fragment，插入到 cursor 前面
                        let effective_cursor = cursor.as_ref().unwrap_or(&end_node);
                        if let Some(parent) = effective_cursor.parent_node() {
                            let _ = parent.insert_before(&frag, Some(effective_cursor));
                        }
                    } else {
                        // 如果是已有项，检查其 DOM 位置
                        if nodes.is_empty() {
                            continue;
                        }

                        let first_node = &nodes[0];
                        let is_in_place = cursor
                            .as_ref()
                            .is_some_and(|c| c.is_same_node(Some(first_node)));

                        if is_in_place {
                            // 已经在正确位置，跳过这组节点
                            for _ in 0..nodes.len() {
                                cursor = cursor.and_then(|c| c.next_sibling());
                            }
                        } else {
                            // 不在正确位置，移动到 cursor 前面
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

/// 一个特殊的视图，它基于索引（Index）来协调列表项。
///
/// 适用于列表项频繁更新但顺序变化较少或不支持稳定 Key 的场景。
pub struct IndexedLoopView<IF, T, IS> {
    pub each: IF,
    pub view_fn: Rc<dyn Fn(ReadSignal<T>, ReadSignal<usize>) -> AnyView + 'static>,
    pub _marker: std::marker::PhantomData<(T, IS)>,
}

impl<IF, T, IS> ApplyAttributes for IndexedLoopView<IF, T, IS> {}

impl<IF, T, IS> View for IndexedLoopView<IF, T, IS>
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = T> + 'static,
    T: Clone + 'static,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_indexed_loop_logic(self.each.clone(), self.view_fn.clone(), parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        mount_indexed_loop_logic(self.each, self.view_fn, parent, attrs);
    }
}

struct IndexedLoopRow<T> {
    item_setter: WriteSignal<T>,
    index_setter: WriteSignal<usize>,
    scope_id: NodeId,
    nodes: Vec<Node>,
}

fn mount_indexed_loop_logic<IF, T, IS>(
    items_fn: IF,
    view_fn: Rc<dyn Fn(ReadSignal<T>, ReadSignal<usize>) -> AnyView + 'static>,
    parent: &Node,
    _attrs: Vec<PendingAttribute>,
) where
    IF: RxRead<Value = IS> + 'static,
    IS: ForLoopSource<Item = T> + 'static,
    T: Clone + 'static,
{
    let document = crate::document();
    let start_node: Node = document.create_comment("indexed-for-start").into();
    let end_node: Node = document.create_comment("indexed-for-end").into();

    let _ = parent.append_child(&start_node);
    let _ = parent.append_child(&end_node);

    let rows = Rc::new(RefCell::new(Vec::<IndexedLoopRow<T>>::new()));

    Effect::new(move |_| {
        items_fn.with(|items| {
            let Ok(items_slice) = items.as_slice() else {
                if let Err(e) = items.as_slice() {
                    silex_core::error::handle_error(e);
                }
                return;
            };

            let mut rows_lock = rows.borrow_mut();

            batch(|| {
                let new_len = items_slice.len();
                let old_len = rows_lock.len();
                let common_len = std::cmp::min(new_len, old_len);

                // 更新共有部分
                for (i, item) in items_slice.iter().take(common_len).enumerate() {
                    rows_lock[i].item_setter.set(item.clone());
                    rows_lock[i].index_setter.set(i);
                }

                // 添加新增项
                if new_len > old_len {
                    for (i, item) in items_slice[common_len..].iter().enumerate() {
                        let (item_setter, index_setter, scope_id, nodes, fragment_node) =
                            untrack(|| {
                                let real_index = common_len + i;
                                let (get, item_setter) = Signal::pair(item.clone());
                                let (index_get, index_setter) = Signal::pair(real_index);
                                let fragment = document.create_document_fragment();
                                let fragment_node: Node = fragment.clone().into();
                                let fragment_node_clone = fragment_node.clone();
                                let view_fn = view_fn.clone();

                                let scope_id = create_scope(move || {
                                    (view_fn)(get, index_get)
                                        .mount_owned(&fragment_node_clone, Vec::new());
                                });

                                let nodes_list = fragment.child_nodes();
                                let len = nodes_list.length();
                                let mut nodes = Vec::with_capacity(len as usize);
                                for j in 0..len {
                                    if let Some(n) = nodes_list.item(j) {
                                        nodes.push(n);
                                    }
                                }
                                (item_setter, index_setter, scope_id, nodes, fragment_node)
                            });

                        // 插入到 end_node 之前
                        if let Some(p) = end_node.parent_node() {
                            let _ = p.insert_before(&fragment_node, Some(&end_node));
                        }

                        rows_lock.push(IndexedLoopRow {
                            item_setter,
                            index_setter,
                            scope_id,
                            nodes,
                        });
                    }
                }

                // 移除多余项
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
