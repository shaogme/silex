# Crate: `silex_reactivity`

`silex_reactivity` 是 Silex 的底层、类型擦除、细粒度响应式引擎。公共 API 按 signal、memo、effect、scope、store、callback 和 node-ref 分模块；句柄带静态种类，跨种类操作需要显式擦除为 `RawId`。

## Runtime

运行时是线程局部的，所有内部访问通过 `with_rt` 获取独占 `&mut Runtime`。用户计算闭包、effect、cleanup 和用户析构不会在该借用内执行。`drive` 模块在两次借用之间驱动同步惰性 memo、effect 队列和 dispose 工作栈。

## Storage SoA

响应式节点拆成独立组件表：

* `graph` 保存节点拓扑。
* `node_aux` 保存 owner 子节点和 cleanup。
* `meta` 保存状态、种类标志、版本和追踪去重缓存。
* `links` 保存订阅边和依赖边。
* `values` 保存 signal/memo 值，`computations` 保存 effect/memo 闭包。
* `extras` 保存非响应式类型擦除载荷。

传播只读 `links`、写 `meta`。依赖边额外保存订阅者槽位，退订使用 `swap_remove`，避免高扇出路径的平方复杂度。DFS 帧使用依赖游标，避免同一节点重复全量扫描。

## Arena

`Arena<T>` 和 `SparseSecondaryMap<T>` 位于 `internal/arena.rs`，使用安全 `Vec`/分块 `Vec`，文件级 `#![forbid(unsafe_code)]`。`RawId` 保持 `u32` 槽位号与 generation，支持复用、旧句柄失效和 stale-generation 写入拒绝。修改入口需要 `&mut self`，因此同表引用不能跨越写操作。

## Remaining Unsafe

arena 已零 unsafe。剩余 unsafe 位于 `ThinVec`、`AnyValue`、vtable 和三个显式原始引用 API。原始引用 API 的调用者必须遵守各自 `# Safety` 契约，不得跨越节点销毁、值替换或再次运行时调用。

## Key Commands

```text
cargo test -p silex_reactivity
cargo test -p silex_reactivity --test graph_cost --release -- --nocapture
cargo bench -p silex_reactivity --bench reactivity
cargo +nightly miri test -p silex_reactivity -p silex_vtable
```
