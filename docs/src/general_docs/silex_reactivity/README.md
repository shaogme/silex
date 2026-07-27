# Silex Reactivity 引擎

`silex_reactivity` 是 Silex 的类型擦除、细粒度响应式运行时，提供 signal、memo、effect、scope 和非响应式载荷管理。

## 核心架构

运行时通过线程局部的 `RefCell<Runtime>` 提供唯一的 `&mut Runtime` 入口。用户闭包在两次运行时借用之间执行，驱动层负责求值、队列、cleanup 和墓园析构。

响应式节点采用 SoA 存储：

* `graph: Arena<Node>` 保存 parent 和调试定义位置。
* `node_aux` 保存 children、cleanup 和调试标签等冷数据。
* `meta` 保存 `NodeState`、种类标志、版本号和依赖追踪缓存。
* `links` 保存 subscribers 与 dependencies。
* `values` 和 `computations` 分别保存可读值与计算闭包。
* `extras` 保存 stored value、callback 和 node-ref 的类型擦除载荷。

`NodeMeta`、`NodeLinks`、值和计算闭包互不嵌套，传播可以只读 `links` 并独占修改 `meta`。用户代码执行前，值或闭包会从对应表中移出，避免跨回调持有运行时内部引用。

## 图算法

`runtime/graph.rs` 直接实现 BFS 传播和带驱动的 DFS 求值，不再通过 `ReactiveGraph` trait 转发。

* 直接订阅者标记为 `Dirty`，更深层节点标记为 `Check`，effect 进入队列。
* DFS 帧带依赖游标；一个有 `k` 条依赖的节点只做增量扫描，避免 O(k²) 重扫。
* 依赖边记录订阅者槽位。退订通过 `swap_remove` 并修正被移动边的反向槽位，扇出更新保持线性。
* memo 仍然惰性求值，`Check` 节点通过版本号跳过无效重算。

## 安全容器

`internal/arena.rs` 使用安全的 `Vec` 槽位和分块 `Vec` 旁路表，文件级 `#![forbid(unsafe_code)]` 保证 arena 本身零 unsafe。`Index` 仍是 `{u32 index, u32 generation}`，支持槽位复用、ABA 防护和 `DANGLING` 句柄。

容器的读取使用共享借用，插入、删除和可变访问使用 `&mut self`。因此 Rust 借用检查器会阻止同一张表的引用跨越写操作；旧代数写入会被拒绝。

## 类型擦除与逃生出口

`AnyValue` 提供小对象优化、类型检查和正确析构。它与 `ThinVec` 仍包含 crate 内部的 unsafe；公开的 `try_get_any_raw_untracked`、`signal::try_value_ref` 和 `store::try_value_ref` 也保留显式 `# Safety` 契约。

## 生命周期

`scope::dispose` 使用显式后序工作栈销毁子树。cleanup、用户 `Drop`、被覆盖的值和计算闭包先进入墓园，再在运行时借用之外析构。`untrack` 只清理 observer，不改变 owner，因此其中创建的节点仍随当前 scope 回收。

## 验证

```text
cargo test -p silex_reactivity
cargo test -p silex_reactivity --test graph_cost --release -- --nocapture
```
