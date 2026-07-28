//! 运行时所有权层级与节点销毁的基础操作。

use crate::{
    DependencyList,
    internal::{arena::RawId, value::OnceThunk},
    runtime::{Runtime, storage::CleanupList, storage::Debris, storage::Node},
};
use std::{mem, panic::Location};

/// 记录当前线程上下文中的**所有权所有者 (Owner)** 与 **依赖观察者 (Observer)**。
///
/// 所有权与依赖追踪完全正交：
/// - `current_owner`: 决定新建节点挂载在哪个父 Scope 之下，以及 `on_cleanup` 回调注册给谁。
/// - `current_observer`: 决定读取 Signal 时将哪个计算节点登记为订阅者。
pub(crate) struct Scopes {
    pub(crate) current_owner: Option<RawId>,
    pub(crate) current_observer: Option<RawId>,
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
    pub(crate) fn current_owner(&self) -> Option<RawId> {
        self.scopes.current_owner
    }

    pub(crate) fn set_owner(&mut self, owner: Option<RawId>) {
        self.scopes.current_owner = owner;
    }

    pub(crate) fn current_observer(&self) -> Option<RawId> {
        self.scopes.current_observer
    }

    pub(crate) fn set_observer(&mut self, observer: Option<RawId>) {
        self.scopes.current_observer = observer;
    }

    pub(crate) fn internal_on_cleanup(&mut self, thunk: OnceThunk) {
        if let Some(owner) = self.current_owner() {
            self.storage
                .with_aux_mut(owner, |aux| aux.cleanups.push(thunk));
        }
    }

    /// 在当前所有权节点下注册并创建新节点。
    ///
    /// `at` 为用户代码所在位置的 `Location`。
    pub(crate) fn register_node_at(&mut self, _at: &'static Location<'static>) -> RawId {
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

    /// 提取并清空指定节点的子节点列表与 Cleanup 清理函数列表。
    pub(crate) fn take_scope_state(&mut self, id: RawId) -> (Vec<RawId>, CleanupList) {
        let Some(aux) = self.storage.node_aux.get_mut(id) else {
            return (Vec::new(), CleanupList::Empty);
        };
        (mem::take(&mut aux.children), mem::take(&mut aux.cleanups))
    }

    /// 提取并清空指定计算节点的依赖节点列表。
    pub(crate) fn take_dependencies(&mut self, id: RawId) -> DependencyList {
        let Some(links) = self.storage.links.get_mut(id) else {
            return DependencyList::default();
        };
        mem::take(&mut links.dependencies)
    }

    /// 将 `self_id` 从其所有依赖节点的订阅者列表 (`subscribers`) 中移除。
    pub(crate) fn unsubscribe(&mut self, self_id: RawId, dependencies: DependencyList) {
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

    /// 从存储中注销并抹除指定节点及其载荷。
    ///
    /// 包含用户自定义类型的载荷（值、闭包、Cleanup 等）将被转移入墓园 ([`Debris`])，
    /// 延后至 Runtime 独占借用释放之后统一析构，防止析构过程重入访问 Runtime。
    pub(crate) fn forget_node(&mut self, id: RawId) {
        #[cfg(debug_assertions)]
        {
            let label = self
                .storage
                .node_aux
                .get_mut(id)
                .and_then(|aux| aux.debug_label.take());
            if let Some(label) = label {
                self.storage.remember_dead_label(id, label);
            }
        }

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
        internal::arena::RawId,
        runtime::{drive, with_rt_or_init},
    };
    use std::{cell::RefCell, rc::Rc};

    /// 在 `owner` 之下注册一个节点，并把 owner 切到它身上。
    fn child_of(owner: RawId) -> RawId {
        with_rt_or_init(|rt| {
            rt.set_owner(Some(owner));
            rt.register_node_at(std::panic::Location::caller())
        })
        .expect("运行时可用")
    }

    fn root_node() -> RawId {
        with_rt_or_init(|rt| rt.register_node_at(std::panic::Location::caller())).expect("可用")
    }

    fn set_owner(owner: Option<RawId>) {
        let _ = with_rt_or_init(|rt| rt.set_owner(owner));
    }

    fn record(owner: RawId, log: &Rc<RefCell<Vec<&'static str>>>, tag: &'static str) {
        set_owner(Some(owner));
        let log = log.clone();
        drive::on_cleanup(move || log.borrow_mut().push(tag));
    }

    fn alive(id: RawId) -> bool {
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

                drive::dispose_raw(root);

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

            drive::dispose_raw(root);

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
                    drive::dispose_raw(b);
                });
            }
            set_owner(None);

            drive::dispose_raw(root);

            assert_eq!(*log.borrow(), vec!["a", "b"]);
            assert!(!alive(b));
            assert!(!alive(root));
        })
        .join()
        .expect("用例在自己的线程里跑");
    }
}
