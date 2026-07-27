//! 所有权上下文与销毁的**纯操作**部分。
//!
//! 会执行用户代码的那一半（`create_scope` / `untrack` / `batch` / `dispose` /
//! cleanup 的驱动循环）住在 [`drive`](crate::runtime::drive) 里 —— 它们必须在
//! 两次借用之间跑用户代码，因此不可能是 `Runtime` 上的方法。

use crate::{
    DependencyList,
    internal::{arena::Index as NodeId, value::OnceThunk},
    runtime::{Runtime, storage::CleanupList, storage::Debris, storage::Node},
};
use std::{mem, panic::Location};

/// 当前的**所有权**上下文与**依赖追踪**上下文。
///
/// 这是两件正交的事，此前被绑在同一个 `current_owner` 上，于是 `untrack`
/// 一清就把所有权也清掉了 —— 在 `untrack` 里创建的节点没有父节点、不在任何
/// scope 的 children 里、永远不会被 `dispose` 回收（AUDIT 二轮 §1.1）。
/// SolidJS 把这两个分成 `Owner` 与 `Listener`，`untrack` 只清 `Listener`。
///
/// | 变量 | 含义 | 谁读它 |
/// |---|---|---|
/// | `current_owner` | 新节点挂在谁下面、`on_cleanup` 注册给谁 | `register_node_at`、`internal_on_cleanup` |
/// | `current_observer` | 读 signal 时把谁登记为订阅者 | `track_dependency` / `track_dependencies` |
pub(crate) struct Scopes {
    pub(crate) current_owner: Option<NodeId>,
    pub(crate) current_observer: Option<NodeId>,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Self {
            current_owner: None,
            current_observer: None,
        }
    }
}

impl Runtime {
    pub(crate) fn current_owner(&self) -> Option<NodeId> {
        self.scopes.current_owner
    }

    pub(crate) fn set_owner(&mut self, owner: Option<NodeId>) {
        self.scopes.current_owner = owner;
    }

    pub(crate) fn current_observer(&self) -> Option<NodeId> {
        self.scopes.current_observer
    }

    pub(crate) fn set_observer(&mut self, observer: Option<NodeId>) {
        self.scopes.current_observer = observer;
    }

    pub(crate) fn internal_on_cleanup(&mut self, thunk: OnceThunk) {
        if let Some(owner) = self.current_owner() {
            self.storage
                .with_aux_mut(owner, |aux| aux.cleanups.push(thunk));
        }
    }

    /// 建一个新节点并挂到当前 owner 下面。
    ///
    /// `at` 由调用方显式传进来，而不是靠 `#[track_caller]` 在这里取 ——
    /// 建节点的入口现在都隔着一层 `with_rt` 的闭包，而 `#[track_caller]`
    /// 穿不过闭包边界，就地取会得到运行时内部的某一行，而不是用户的调用点
    /// （AUDIT P11）。
    pub(crate) fn register_node_at(&mut self, _at: &'static Location<'static>) -> NodeId {
        let parent = self.current_owner();
        let mut node = Node::new();
        node.parent = parent;

        #[cfg(debug_assertions)]
        {
            node.defined_at = Some(_at);
        }

        let id = self.storage.graph.insert(node);

        if let Some(parent_id) = parent {
            self.storage
                .with_aux_mut(parent_id, |aux| aux.children.push(id));
        }
        id
    }

    /// 摘下一个节点的子节点列表与 cleanup 列表。
    pub(crate) fn take_scope_state(&mut self, id: NodeId) -> (Vec<NodeId>, CleanupList) {
        let Some(aux) = self.storage.node_aux.get_mut(id) else {
            return (Vec::new(), CleanupList::Empty);
        };
        (mem::take(&mut aux.children), mem::take(&mut aux.cleanups))
    }

    /// 摘下一个计算节点的依赖列表。
    pub(crate) fn take_dependencies(&mut self, id: NodeId) -> DependencyList {
        let Some(links) = self.storage.links.get_mut(id) else {
            return DependencyList::default();
        };
        mem::take(&mut links.dependencies)
    }

    /// 把 `self_id` 从它所有依赖的订阅者表里摘掉。
    pub(crate) fn unsubscribe(&mut self, self_id: NodeId, dependencies: DependencyList) {
        for (dep_id, _, subscriber_index) in dependencies {
            let Some((moved_id, last_index)) =
                self.storage.links.get_mut(dep_id).and_then(|links| {
                    let index = if links.subscribers.get(subscriber_index) == Some(&self_id) {
                        subscriber_index
                    } else {
                        links.subscribers.iter().position(|&id| id == self_id)?
                    };
                    let last_index = links.subscribers.len() - 1;
                    links.subscribers.swap_remove(index);
                    (index < last_index).then(|| (links.subscribers[index], last_index))
                })
            else {
                continue;
            };

            if let Some(moved_links) = self.storage.links.get_mut(moved_id) {
                for edge in moved_links.dependencies.as_mut_slice() {
                    if edge.0 == dep_id && edge.2 == last_index {
                        edge.2 = subscriber_index;
                        break;
                    }
                }
            }
        }
    }

    /// 把节点本身从所有存储中抹掉（cleanup 已经跑过、订阅已经解除）。
    ///
    /// 摘下来的载荷（值、计算闭包、尚未执行的 cleanup）**不在这里析构** ——
    /// 它们装的是用户数据，析构就是执行用户的 `Drop`，而用户的 `Drop` 可以
    /// 回头访问响应式图。一律推进墓园，由调用方在借用之外排空
    /// （见 [`Debris`] 与 [`drain_graveyard`](crate::runtime::drive::drain_graveyard)）。
    pub(crate) fn forget_node(&mut self, id: NodeId) {
        #[cfg(debug_assertions)]
        {
            // 标签先摘出来再登记墓碑：`remember_dead_label` 要写另一张表，
            // 不该在 `node_aux` 的借用还活着的时候进行。
            let label = self
                .storage
                .node_aux
                .get_mut(id)
                .and_then(|aux| aux.debug_label.take());
            if let Some(label) = label {
                self.storage.remember_dead_label(id, label);
            }
        }

        // `Node` 自己只有 parent 与定义位置，不含用户数据，可以就地析构。
        self.storage.graph.remove(id);
        if let Some(aux) = self.storage.node_aux.remove(id) {
            self.storage.bury(Debris::Aux(aux));
        }
        if let Some(node) = self.storage.remove_reactive(id) {
            self.storage.bury(Debris::Node(node));
        }
        if let Some(payload) = self.storage.extras.remove(id)
            && let Some(value) = payload
        {
            self.storage.bury(Debris::Payload(value));
        }
        self.scheduler.queued_observers.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        internal::arena::Index as NodeId,
        runtime::{drive, with_rt_or_init},
    };
    use std::{cell::RefCell, rc::Rc};

    /// 在 `owner` 之下注册一个节点，并把 owner 切到它身上。
    fn child_of(owner: NodeId) -> NodeId {
        with_rt_or_init(|rt| {
            rt.set_owner(Some(owner));
            rt.register_node_at(std::panic::Location::caller())
        })
        .expect("运行时可用")
    }

    fn root_node() -> NodeId {
        with_rt_or_init(|rt| rt.register_node_at(std::panic::Location::caller())).expect("可用")
    }

    fn set_owner(owner: Option<NodeId>) {
        let _ = with_rt_or_init(|rt| rt.set_owner(owner));
    }

    fn record(owner: NodeId, log: &Rc<RefCell<Vec<&'static str>>>, tag: &'static str) {
        set_owner(Some(owner));
        let log = log.clone();
        drive::on_cleanup(move || log.borrow_mut().push(tag));
    }

    fn alive(id: NodeId) -> bool {
        with_rt_or_init(|rt| rt.storage.graph.get(id).is_some()).unwrap_or(false)
    }

    /// 深链销毁不得爆栈（AUDIT P19.8）。
    ///
    /// 特意跑在一个小栈线程里：递归实现在这个深度必然溢出，
    /// 而显式工作栈只吃堆，栈深度是常数。
    ///
    /// 这个用例要建满五万个节点才有意义，在 Miri 下慢得没有价值，
    /// 而它考察的是遍历形态、不涉及任何 `unsafe` 边界，因此 Miri 下跳过。
    #[test]
    #[cfg_attr(miri, ignore)]
    fn disposing_a_deep_tree_does_not_overflow_the_stack() {
        const DEPTH: usize = 50_000;

        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut ids = Vec::with_capacity(DEPTH);
                let root = root_node();
                ids.push(root);
                let mut owner = root;
                for _ in 1..DEPTH {
                    owner = child_of(owner);
                    ids.push(owner);
                }
                set_owner(None);

                drive::dispose(root);

                for id in ids {
                    assert!(!alive(id));
                }
            })
            .expect("spawn")
            .join()
            .expect("深链销毁不应爆栈");
    }

    /// 后序遍历的顺序必须与原递归实现逐字一致：
    /// 孙子先于儿子、子树先于自身、同级按注册顺序。
    #[test]
    fn cleanup_order_is_depth_first_post_order() {
        std::thread::spawn(|| {
            let log = Rc::new(RefCell::new(Vec::new()));

            //        root
            //       /    \
            //      a      b
            //     / \
            //   a1   a2
            let root = root_node();
            record(root, &log, "root");

            let a = child_of(root);
            record(a, &log, "a");
            let a1 = child_of(a);
            record(a1, &log, "a1");
            let a2 = child_of(a);
            record(a2, &log, "a2");

            let b = child_of(root);
            record(b, &log, "b");
            set_owner(None);

            drive::dispose(root);

            assert_eq!(*log.borrow(), vec!["a1", "a2", "a", "b", "root"]);
        })
        .join()
        .expect("用例在自己的线程里跑，免得与别的用例共用同一份运行时");
    }

    /// cleanup 在销毁途中把一棵尚未处理的兄弟子树先销毁掉：
    /// 那棵子树的 cleanup 只跑一次，工作栈之后碰到它时安静跳过。
    #[test]
    fn cleanup_may_dispose_a_pending_sibling() {
        std::thread::spawn(|| {
            let log = Rc::new(RefCell::new(Vec::new()));

            let root = root_node();
            // 先把 b 建出来，好让 a 的 cleanup 能捕获它的 id。
            let a = child_of(root);
            let b = child_of(root);

            record(b, &log, "b");

            set_owner(Some(a));
            {
                let log = log.clone();
                drive::on_cleanup(move || {
                    log.borrow_mut().push("a");
                    drive::dispose(b);
                });
            }
            set_owner(None);

            drive::dispose(root);

            assert_eq!(*log.borrow(), vec!["a", "b"]);
            assert!(!alive(b));
            assert!(!alive(root));
        })
        .join()
        .expect("用例在自己的线程里跑");
    }
}
