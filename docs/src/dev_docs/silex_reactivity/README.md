# `silex_reactivity` 内部开发文档

## 设计边界

运行时通过 `with_rt` 交出独占的 `&mut Runtime`。内部方法只计算状态、改图和准备驱动步骤，不执行用户代码；effect、memo、cleanup、析构和 `PartialEq` 都在两次运行时借用之间执行。

业务字段和公开入口保存具体句柄；只有 owner/observer、订阅边、依赖边、工作队列以及
非泛型 trampoline 这类异构结构使用 `RawId`。`Handle::into_raw()` 是显式擦除入口，
`from_raw_unchecked()` 只能出现在已有运行时种类证明的 dispatcher、适配器和测试探针中。

### 当前模块

```text
src/
├── runtime.rs            // Runtime、依赖追踪和读路径
├── runtime/drive.rs      // 求值、队列、scope 销毁和用户代码边界
├── runtime/graph.rs      // SoA 上的 BFS/DFS 图算法
├── runtime/storage.rs    // NodeMeta、NodeLinks、值/闭包表和墓园
├── runtime/guard.rs      // owner、observer、借出值和调度状态守卫
├── internal/arena.rs     // 零 unsafe 的 Arena / SparseSecondaryMap
├── internal/list.rs      // ThinVec / List，仍是内部 unsafe 热点
└── internal/value.rs     // AnyValue 与计算闭包的类型擦除
```

## SoA 存储

`Storage` 把响应式节点拆成互不重叠的表：

```text
graph          Arena<Node>
node_aux       SparseSecondaryMap<NodeAux>
meta           SparseSecondaryMap<NodeMeta>
links          SparseSecondaryMap<NodeLinks>
values         SparseSecondaryMap<Option<AnyValue>>
computations   SparseSecondaryMap<Option<Computation>>
extras         SparseSecondaryMap<Option<AnyValue>>
```

传播只借用 `links` 并修改 `meta`，所以不需要节点内 `Cell`/`RefCell`。值或闭包在调用用户代码前通过 `take` 移出，守卫在正常返回或 panic 时放回。

`NodeLinks.subscribers` 是 `Vec<RawId>`。依赖边保存 `(target, version, subscriber_index)`，退订使用 `swap_remove`，并更新被搬移订阅者依赖列表里的反向索引。这避免了一个 signal 有 `k` 个 memo 时逐个线性查找订阅者造成的 O(k²)。

## 图算法

`runtime/graph.rs` 直接实现算法，不再存在 `GraphStorage`、`GraphScheduler`、`GraphExecutor`、`ReactiveGraph` 或 `RuntimeAdapter`。

`EvalFrame.cursor` 记录依赖扫描位置。扫描到末尾后最多做一次从头复查，用于保留重入写入语义；正常情况下每条依赖只访问一次。

## 安全容器

`internal/arena.rs` 由普通 `Vec` 槽位和分块 `Vec` 条目构成，并使用 `#![forbid(unsafe_code)]`。写入口为 `&mut self`，句柄字段保持私有，代数校验保持旧句柄失效和 ABA 防护。

arena 之外的 unsafe 仍集中在 `internal/list.rs`、`internal/value.rs`、vtable 和公开 raw-reference 逃生出口。不要把 raw pointer 重新引入 arena 来优化跨借用访问；需要并行访问时拆分 Storage 字段或先复制轻量句柄。

## 验证命令

```text
cargo test -p silex_reactivity
cargo test -p silex_reactivity --release
cargo bench -p silex_reactivity --bench reactivity
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly miri test -p silex_reactivity -p silex_vtable
```
