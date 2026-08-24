+++
title = "控制流与列表"
description = "silex 的响应式条件、动态 view、分支和列表组件。"
weight = 10
+++

# 控制流与列表

`silex::flow` 把 `silex_core` 的 `ReactiveSource` 输入连接到
`silex_view` 的动态 view 和 list kernel。组件自身只负责把 builder props
转换成 owner-bound view；DOM range、row owner、失败回滚和 stale updater 仍由
`silex_view` 执行，物理 tree 操作由注入的 `silex_dom` backend 提供。使用这些组件前，先确认 source 与 view 属于兼容的
`'scope`/runtime。

本页的短代码块是 API 契约片段，依赖外层 `ctx`、owner 和 source，不是独立的
CI 编译示例；可执行 facade 示例见总文档引用的 `docs/examples/silex/basic.rs`。

## 条件与动态 view

### `Show`

`Show(ctx, when)` 接收值为 `bool` 的 `ReactiveSource`，通过 `.children(view)`
设置真分支，通过 `.fallback(view)` 设置假分支；fallback 默认是空 view。
`SignalShowExt::when` 是等价的 builder 语法糖：

```rust
let view = visible
    .when(ctx, div("visible"))
    .fallback(div("hidden"))
    .build();
```

这里的 `visible` 可以是 `Signal`、computed 或其它满足 `ReactiveSource` 的
输入。`Show` 会在创建阶段通过 context owner promotion 输入，之后由 reactive
view 订阅变化；不要把一个属于另一个 runtime 的 tracked source 传进来。

### `Dynamic`

`Dynamic(ctx, view_fn)` 用 `ReactiveSource<Value = V>` 产生当前 view。`V` 需要
实现 `View<'scope> + Clone`，因此适合把 signal/computed 生成的不同元素或
`AnyView` 分支放到同一个位置：

```rust
let dynamic = Dynamic(ctx, current_view)
    .build();
```

`Dynamic` 只改变当前动态范围内的 view，不复用已经 mount 的 `MountInstance`。
需要让不同 Rust 类型的分支共存时，可在 source 中使用 `view_match!` 或显式
调用 `.into_any()`；类型擦除不会改变 owner cleanup 和错误 handler。

动态 view 的首次 render 失败会清理 provisional range；后续 render 失败保留
前一次成功内容并把错误交给 handler。这是 DOM kernel 的提交契约，不应通过在
闭包中记录日志后返回一个假 view 来绕过。

## `Switch`

`Switch(ctx, source)` 以 `Eq + Hash + Clone` 的 source value 查找分支。分支通过
`.case(value, view)` 添加，未匹配的值使用 `.fallback(view)`：

```rust
let tabs = Switch(ctx, active_tab)
    .fallback("unknown tab")
    .build()
    .case("home", div("Home"))?
    .case("settings", div("Settings"))?;
```

`.case` 返回 `Result<Self, SilexError>`。同一个 key 第二次加入时返回 fatal
framework/javascript 错误，不会静默覆盖旧分支。分支 view 保存为
`AnyView<'scope>`，所以不同元素类型可以共用一个 `Switch`。

## 列表组件

三个列表组件的选择取决于 row identity，而不是只取决于当前渲染结果：

| 组件 | identity | children 回调 | 适用场景 |
| --- | --- | --- | --- |
| `Index` | 位置/index | `Fn(Item, usize) -> View` | 位置就是 identity，重排不需要保留 item identity。 |
| `For` | `key(&Item)` | `Fn(Item, usize) -> View` | key 稳定但每次 item/index 变化都允许重新渲染 row。 |
| `ForStateful` | `key(&Item)` | `Fn(Item, usize, RowUpdater<Item>) -> View` | key 相同时保留 row owner 和局部状态。 |

`each` 必须是 core 支持的 `ForLoopSource` 输入，并同时满足列表组件要求的
`RxRead` 与 `ReactiveSource`。实际调用中的闭包参数由
`#[prop(render_fn(...))]` 推导，所以不要手动给 `children` 闭包添加一个不匹配
的类型擦除层：

```rust
let list = For(ctx, items, |item| item.id)
    .children(|item, index| li(format!("{index}: {}", item.title)))
    .row_error_handler(row_handler)
    .build();
```

### `For` 与 `ForStateful`

`For` 内部使用 `RenderOnlyKeyedListView`。同 key 的 row 仍经过 render-only
路径；应用不应把它当作 stateful row。`ForStateful` 使用
`StatefulKeyedListView`，`RowUpdater` 只能为当前 generation 绑定一次 callback：

- row active 且 callback 已绑定时，`update(item, index)` 返回 `true`；
- row 被删除、key generation 失效或 callback 尚未绑定时返回 `false`；
- updater 不应被保存为全局 setter；它失效后保持 inert 是防止 stale row 写入
  新 DOM 的必要保护；
- callback panic 会恢复后重新抛出，不会被当成普通 row 更新失败。

初始或更新列表出现 duplicate key 时，列表返回 framework error，并回滚尚未提交
的 rows。row factory 的错误应交给 `row_error_handler`；不要用一个外层
completion endpoint 代替 owner-bound error handler。

### `Index`

`Index` 不调用 key function，而是按位置更新 row 输入。它适合基础类型或没有
稳定唯一 key 的列表：

```rust
let list = Index(ctx, values)
    .children(|value, index| div(format!("{index}: {value}")))
    .build();
```

如果列表中的元素会插入、删除或重排并且每项拥有局部状态，应优先选择 keyed
`ForStateful`；否则位置 identity 会把旧 row 的状态留在新位置上。

## 生命周期和失败路径

flow 组件创建的动态范围和列表 row 都属于当前 mount owner。更新时先准备新的
range/rows，提交失败时保留旧内容；partial child mount 会关闭 provisional owner
并处理 detached DOM。branch key 改变时旧 content owner 会被关闭，key 相同时
会保留稳定 branch runtime。

这些行为意味着：

- `View`/`AnyView` 是 factory，不要缓存已经挂载的 DOM 节点来模拟复用；
- `RowUpdater`、callback 和 source 必须不超出 `'scope`，也不能跨 runtime；
- `SilexError` 必须传播到传入的 handler；错误后继续使用的内容是否保留由
  `silex_dom` 的动态 kernel 决定；
- 调试时同时记录 key、row generation、当前 range 和 owner close 顺序，不要只
  检查最终 text content。

## 源码与测试

- 实现：`crates/silex/src/flow/dynamic.rs`、`for_loop.rs`、`index.rs`、
  `show.rs`、`switch.rs`
- children 类型推导：`crates/silex/tests/for_children.rs` 和
  `tests/ui/pass_for_children_field_access.rs`
- 列表 identity、rollback 和 updater：`crates/silex_dom/tests/owner.rs`、
  `tests/mounted_app.rs` 以及 [`silex_dom` 视图文档](@/developer/crates/silex_dom/views.md)
- 宏字段与 render function：[`silex_macros component`](@/developer/crates/silex_macros/component.md)

页面中的短片段只展示 API 契约；可执行的 facade 示例位于
[`silex` 总文档](@/developer/crates/silex/_index.md)引用的
`docs/examples/silex/basic.rs`。
