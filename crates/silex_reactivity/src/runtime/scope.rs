use crate::{
    DependencyList,
    core::{arena::Index as NodeId, value::OnceThunk},
    runtime::{
        Runtime,
        guard::{ObserverGuard, OwnerGuard},
        storage::{CleanupList, Node},
    },
};
use std::{
    cell::{Cell, RefCell},
    mem,
};

/// 复用池里最多保留多少个销毁工作栈。
///
/// 与 [`crate::runtime::scheduler::WorkSpace`] 的池同一个量级：销毁可以重入
/// （cleanup 闭包里再调 `dispose`），但嵌套层数在真实代码里是个位数。
const MAX_POOLED_DISPOSE_STACKS: usize = 32;

/// 当前的**所有权**上下文与**依赖追踪**上下文。
///
/// 这是两件正交的事，此前被绑在同一个 `current_owner` 上，于是 `untrack`
/// 一清就把所有权也清掉了 —— 在 `untrack` 里创建的节点没有父节点、不在任何
/// scope 的 children 里、永远不会被 `dispose` 回收（AUDIT 二轮 §1.1）。
/// SolidJS 把这两个分成 `Owner` 与 `Listener`，`untrack` 只清 `Listener`。
///
/// | 变量 | 含义 | 谁读它 |
/// |---|---|---|
/// | `current_owner` | 新节点挂在谁下面、`on_cleanup` 注册给谁 | `register_node`、`internal_on_cleanup` |
/// | `current_observer` | 读 signal 时把谁登记为订阅者 | `track_dependency` / `track_dependencies` |
pub(crate) struct Scopes {
    pub(crate) current_owner: Cell<Option<NodeId>>,
    pub(crate) current_observer: Cell<Option<NodeId>>,
    /// 销毁工作栈的复用池，避免每次销毁子树都新分配一个 `Vec`。
    dispose_stacks: RefCell<Vec<Vec<DisposeStep>>>,
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Self {
            current_owner: Cell::new(None),
            current_observer: Cell::new(None),
            dispose_stacks: RefCell::new(Vec::new()),
        }
    }
}

/// 显式销毁工作栈的一帧（AUDIT P19.8）。
///
/// 销毁本质上是一次后序遍历：先递归销毁子树，再跑自己的 cleanup。原来的实现
/// 直接用调用栈来表达这个"后序"，递归深度等于组件树深度，深层树会栈溢出。
/// 这里把调用栈搬到堆上：`Enter` 负责下降（摘下 children 并把它们排进工作栈），
/// `Exit` 负责上升（跑 cleanup、退订、抹除节点）。
enum DisposeStep {
    /// 下降：摘下节点的 children / cleanups / dependencies，把子树排进工作栈。
    Enter(NodeId),
    /// 上升：子树已经全部销毁，轮到节点自己。
    ///
    /// cleanups 与 dependencies 在 `Enter` 阶段就已经摘下来随帧带走 —— 这一点很
    /// 关键：cleanup 闭包可能反过来销毁本节点，届时它读到的是一份空列表，
    /// 不会把同一批 cleanup 跑第二遍。
    Exit {
        id: NodeId,
        cleanups: CleanupList,
        dependencies: DependencyList,
    },
}

impl Runtime {
    pub(crate) fn current_owner(&self) -> Option<NodeId> {
        self.scopes.current_owner.get()
    }

    pub(crate) fn set_owner(&self, owner: Option<NodeId>) {
        self.scopes.current_owner.set(owner);
    }

    pub(crate) fn current_observer(&self) -> Option<NodeId> {
        self.scopes.current_observer.get()
    }

    pub(crate) fn set_observer(&self, observer: Option<NodeId>) {
        self.scopes.current_observer.set(observer);
    }

    /// 在 `f` 执行期间关闭依赖追踪 —— **只关追踪**。
    ///
    /// 所有权上下文原封不动：`f` 里创建的节点照旧挂在当前 owner 下面，随它一起
    /// 销毁。之前这里连 owner 一起清掉，于是 `untrack(|| store_value(v))` 这种
    /// “只是想避免建立依赖” 的写法会造出一个永远回收不掉的孤儿节点，而
    /// `silex_core` 的每一个 `Rx::new_op` / `new_constant` 都是这么写的
    /// （AUDIT 二轮 §1.1）。
    pub(crate) fn untrack<T>(&self, f: impl FnOnce() -> T) -> T {
        // 守卫保证 observer 一定会被恢复：裸写法在 f panic 时会让追踪永久关闭
        // （AUDIT P2）。
        let _observer = ObserverGuard::set(self, None);
        f()
    }

    #[track_caller]
    pub(crate) fn create_scope<F>(&self, f: F) -> NodeId
    where
        F: FnOnce(),
    {
        let id = self.register_node();
        let _owner = OwnerGuard::set(self, Some(id));
        // scope 只是一个所有权容器，它自己不是计算节点，没有“重跑”这回事，
        // 因此不能成为 observer。里面的读取一律不建立依赖 —— 这与拆分之前的
        // 行为一致（那时靠 `track_dependency` 检查 owner 有没有 `effect` 来兜底
        // 过滤，现在由类型之外的这一行显式表达）。
        let _observer = ObserverGuard::set(self, None);
        f();
        id
    }

    pub(crate) fn on_cleanup(&self, f: impl FnOnce() + 'static) {
        self.internal_on_cleanup(OnceThunk::new(f))
    }

    pub(crate) fn internal_on_cleanup(&self, thunk: OnceThunk) {
        if let Some(owner) = self.current_owner()
            && let Some(aux) = self.storage.try_aux_mut(owner)
        {
            aux.cleanups.push(thunk);
        }
    }

    pub(crate) fn dispose(&self, id: NodeId) {
        self.dispose_node_internal(id, true);
    }

    #[track_caller]
    pub(crate) fn register_node(&self) -> NodeId {
        let parent = self.current_owner();
        let mut node = Node::new();
        node.parent = parent;

        #[cfg(debug_assertions)]
        {
            node.defined_at = Some(std::panic::Location::caller());
        }

        let id = self.storage.graph.insert(node);

        if let Some(parent_id) = parent
            && let Some(aux) = self.storage.try_aux_mut(parent_id)
        {
            aux.children.push(id);
        }
        id
    }

    pub(crate) fn clean_node(&self, id: NodeId) {
        if self.storage.graph.get(id).is_none() {
            return;
        }
        let (children, cleanups) = self.take_scope_state(id);
        let dependencies = self.take_dependencies(id);

        self.run_cleanups(id, children, cleanups, dependencies);
    }

    /// 摘下一个节点的子节点列表与 cleanup 列表。
    pub(crate) fn take_scope_state(&self, id: NodeId) -> (Vec<NodeId>, CleanupList) {
        match self.storage.node_aux.get_mut(id) {
            Some(aux) => (mem::take(&mut aux.children), mem::take(&mut aux.cleanups)),
            None => (Vec::new(), CleanupList::default()),
        }
    }

    /// 摘下一个计算节点的依赖列表。
    fn take_dependencies(&self, id: NodeId) -> DependencyList {
        if let Some(n) = self.storage.reactive.get_mut(id)
            && let Some(effect_data) = &mut n.effect
        {
            mem::take(&mut effect_data.dependencies)
        } else {
            DependencyList::default()
        }
    }

    pub(crate) fn run_cleanups(
        &self,
        self_id: NodeId,
        children: Vec<NodeId>,
        cleanups: CleanupList,
        dependencies: DependencyList,
    ) {
        // 顺序与原先的递归实现完全一致：子树先于自身，同级按注册顺序，
        // 自身的 cleanup 跑完之后才解除订阅。
        self.dispose_subtrees(children);
        for cleanup in cleanups {
            cleanup.call();
        }
        self.unsubscribe(self_id, dependencies);
    }

    /// 把 `self_id` 从它所有依赖的订阅者表里摘掉。
    fn unsubscribe(&self, self_id: NodeId, dependencies: DependencyList) {
        for (dep_id, _) in dependencies {
            if let Some(n) = self.storage.reactive.get_mut(dep_id)
                && let Some(signal_data) = &mut n.signal
            {
                signal_data.subscribers.remove(&self_id);
            }
        }
    }

    /// 用显式工作栈销毁若干棵子树（含根），栈深度不再受组件树深度限制（AUDIT P19.8）。
    ///
    /// 遍历顺序严格等价于原来的递归：对每个节点，先按注册顺序逐棵销毁子树，
    /// 再跑自己的 cleanup、退订、抹除自身。注意"摘下 children"这一步也是**惰性**的
    /// —— 只有轮到某个节点被 `Enter` 时才去读它的 children，因此前一个兄弟的 cleanup
    /// 闭包对后一个兄弟做的任何改动都仍然可见，和递归时一模一样。
    fn dispose_subtrees(&self, roots: Vec<NodeId>) {
        if roots.is_empty() {
            // 绝大多数节点没有子节点（effect 每次重跑都会走到这里），不碰池子。
            return;
        }

        let mut stack = self.borrow_dispose_stack();
        // 逆序压栈，弹出时才是注册顺序。
        stack.extend(roots.into_iter().rev().map(DisposeStep::Enter));

        while let Some(step) = stack.pop() {
            match step {
                DisposeStep::Enter(id) => {
                    if self.storage.graph.get(id).is_none() {
                        continue;
                    }
                    let (children, cleanups) = self.take_scope_state(id);
                    let dependencies = self.take_dependencies(id);
                    stack.push(DisposeStep::Exit {
                        id,
                        cleanups,
                        dependencies,
                    });
                    stack.extend(children.into_iter().rev().map(DisposeStep::Enter));
                }
                DisposeStep::Exit {
                    id,
                    cleanups,
                    dependencies,
                } => {
                    for cleanup in cleanups {
                        cleanup.call();
                    }
                    self.unsubscribe(id, dependencies);
                    // 子节点不需要从父节点的 children 里摘除：父节点的那份列表
                    // 早在 `Enter` 阶段就被整体 take 走了。
                    self.forget_node(id);
                }
            }
        }

        self.return_dispose_stack(stack);
    }

    fn borrow_dispose_stack(&self) -> Vec<DisposeStep> {
        self.scopes
            .dispose_stacks
            .borrow_mut()
            .pop()
            .unwrap_or_default()
    }

    fn return_dispose_stack(&self, mut stack: Vec<DisposeStep>) {
        stack.clear();
        let mut pool = self.scopes.dispose_stacks.borrow_mut();
        if pool.len() < MAX_POOLED_DISPOSE_STACKS {
            pool.push(stack);
        }
    }

    /// 把节点本身从所有存储中抹掉（cleanup 已经跑过、订阅已经解除）。
    fn forget_node(&self, id: NodeId) {
        #[cfg(debug_assertions)]
        {
            if let Some(aux) = self.storage.node_aux.get_mut(id)
                && let Some(label) = aux.debug_label.take()
            {
                self.storage.remember_dead_label(id, label);
            }
        }

        self.storage.graph.remove(id);
        self.storage.node_aux.remove(id);
        self.storage.reactive.remove(id);
        self.storage.extras.remove(id);
        self.scheduler.queued_observers.remove(id);
    }

    pub(crate) fn dispose_node_internal(&self, id: NodeId, remove_from_parent: bool) {
        if self.storage.graph.get(id).is_none() {
            return;
        }
        // `clean_node` 内部已经改成显式工作栈，这里不再有任何递归。
        self.clean_node(id);

        if remove_from_parent
            && let Some(parent_id) = self.storage.graph.get(id).and_then(|n| n.parent)
            && let Some(parent_aux) = self.storage.node_aux.get_mut(parent_id)
            && let Some(idx) = parent_aux.children.iter().position(|&x| x == id)
        {
            parent_aux.children.swap_remove(idx);
        }

        self.forget_node(id);
    }
}

#[cfg(test)]
mod tests {
    use crate::{core::arena::Index as NodeId, runtime::Runtime};
    use std::{cell::RefCell, rc::Rc};

    /// 在 `owner` 之下注册一个节点，并把 owner 切到它身上。
    fn child_of(rt: &Runtime, owner: NodeId) -> NodeId {
        rt.set_owner(Some(owner));
        rt.register_node()
    }

    fn record(
        rt: &Runtime,
        owner: NodeId,
        log: &Rc<RefCell<Vec<&'static str>>>,
        tag: &'static str,
    ) {
        rt.set_owner(Some(owner));
        let log = log.clone();
        rt.on_cleanup(move || log.borrow_mut().push(tag));
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
                let rt = Runtime::new();
                let mut ids = Vec::with_capacity(DEPTH);
                let root = rt.register_node();
                ids.push(root);
                let mut owner = root;
                for _ in 1..DEPTH {
                    owner = child_of(&rt, owner);
                    ids.push(owner);
                }
                rt.set_owner(None);

                rt.dispose(root);

                for id in ids {
                    assert!(rt.storage.graph.get(id).is_none());
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
        let rt = Runtime::new();
        let log = Rc::new(RefCell::new(Vec::new()));

        //        root
        //       /    \
        //      a      b
        //     / \
        //   a1   a2
        let root = rt.register_node();
        record(&rt, root, &log, "root");

        let a = child_of(&rt, root);
        record(&rt, a, &log, "a");
        let a1 = child_of(&rt, a);
        record(&rt, a1, &log, "a1");
        let a2 = child_of(&rt, a);
        record(&rt, a2, &log, "a2");

        let b = child_of(&rt, root);
        record(&rt, b, &log, "b");
        rt.set_owner(None);

        rt.dispose(root);

        assert_eq!(*log.borrow(), vec!["a1", "a2", "a", "b", "root"]);
    }

    /// cleanup 在销毁途中把一棵尚未处理的兄弟子树先销毁掉：
    /// 那棵子树的 cleanup 只跑一次，工作栈之后碰到它时安静跳过。
    #[test]
    fn cleanup_may_dispose_a_pending_sibling() {
        let rt = Rc::new(Runtime::new());
        let log = Rc::new(RefCell::new(Vec::new()));

        let root = rt.register_node();
        // 先把 b 建出来，好让 a 的 cleanup 能捕获它的 id。
        let a = child_of(&rt, root);
        let b = child_of(&rt, root);

        record(&rt, b, &log, "b");

        rt.set_owner(Some(a));
        {
            let log = log.clone();
            let rt2 = rt.clone();
            rt.on_cleanup(move || {
                log.borrow_mut().push("a");
                rt2.dispose(b);
            });
        }
        rt.set_owner(None);

        rt.dispose(root);

        assert_eq!(*log.borrow(), vec!["a", "b"]);
        assert!(rt.storage.graph.get(b).is_none());
        assert!(rt.storage.graph.get(root).is_none());
    }
}
