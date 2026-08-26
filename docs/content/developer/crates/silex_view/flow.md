+++
title = "动态 View、分支与列表"
description = "说明 silex_view 的动态 renderer、稳定分支、响应式 View 和 indexed/keyed list。"
weight = 60
+++

# 动态 View、分支与列表

`silex_view::flow` 提供把 `silex_core` reactive source 接入 View tree 的组合器。
它们共享 comment anchor、row owner 和 `MountContext`，所以动态替换不会依赖扫描
真实 DOM 来猜测节点归属；每个 row 都记录自己的连续节点范围和生命周期。

## 动态 View

实现 `Fn() -> V` 的闭包本身实现 `View`。mount 时框架把闭包包装为
`DynamicRenderer`，在自己的 effect 中执行 factory 并把返回 View 渲染到一对
comment anchors 之间。factory 读取的 signal 会成为 effect 依赖，source 更新时
旧 row content 关闭并替换为新的 View。

需要访问完整 `MountContext` 时直接使用 `DynamicRenderer::new`：

```rust
let renderer = DynamicRenderer::new(move |context| {
    let label = value.get()?;
    context.mount(&Element::with_child("output", label))
});
context.mount_unit(renderer, handler.view())?;
```

这是 mount 回调中的片段。`DynamicRenderer` callback 返回
`SilexResult<MountInstance>`，可以使用 `context.mount` 挂载一个 View；renderer
自身返回的 instance 是 anchor range，不应在业务代码中手动移除。

SSR 中动态 renderer 的边界是 comment，例如 `<!--dyn-start-->` 和
`<!--dyn-end-->`；browser 中同样保留逻辑 range，但 serializer 不参与运行时更新。

## `StableBranch`

`StableBranch::new(key_fn, branch_fn)` 要求 `key_fn` 返回
`SilexResult<BranchEvaluation<K, S>>`。`BranchEvaluation::new(key, snapshot)` 的
相等性只比较 `K`，不比较 snapshot：

- key 未变：保留当前 branch content，不重新执行 `branch_fn`；
- key 改变：关闭旧 branch owner，创建新 range row，并用新的 evaluation 渲染；
- key function 出错：在取出 branch state 之前返回错误，因此保留旧 state、旧 row
  和旧 key，并将错误交给 owner 的 error handler。

因此 `snapshot` 是给“key 改变时的新 branch”使用的渲染数据，不是同 key 下的
普通 reactive update 通道。若同一个 key 下的内容也必须更新，应在 branch View
内部使用 signal/dynamic View，或者直接使用 `DynamicRenderer`。

`branch_fn` 还收到 `BranchRenderContext`，可取得 `owner()`、`content_owner()` 和
`error_handler()`。branch content owner 与外层 owner 分开关闭，避免旧 branch 的
effect、NodeRef 和 listener 影响新 branch。

## Indexed list

`IndexedListView::new(each, view_fn)` 按位置维护 row。`each` 需要满足
`RxRead`/`RxReadRef` 和 `ForLoopSource`，`view_fn` 接收 `(item, index)` 并返回
`AnyView`。source 更新时：

- 仍存在的前缀 row 按 index 更新；
- 新长度更大时，在 range end 前创建 row；
- 新长度更小时，移除尾部 row；
- row render 失败时恢复已经更新的旧 snapshot，并清理 pending row。

Indexed list 适合“位置就是身份”的数据。改变一个位置的 item 会重新渲染该 row
content，不提供 key-based identity 保留。

```rust
let list = IndexedListView::new(
    values,
    Rc::new(|value: i32, index| {
        AnyView::from(Element::with_child("li", format!("{index}:{value}")))
    }),
);
context.mount_unit(list, handler.view())?;
```

## Keyed list

`RenderOnlyKeyedListView::new(each, key_fn, view_fn, error_handler)` 和
`StatefulKeyedListView::new(each, key_fn, view_fn, error_handler)` 都要求 key
实现 `Hash + Eq + Clone`；每次 source 更新时先计算所有 key，并拒绝重复 key。
keyed reconcile 会按 key 找到旧 row，再把 row 的连续 range 移到新顺序；删除的
row 会关闭 owner 并移除 range，新 row 先在 detached fragment 中创建，再移动到
目标位置。

两种 keyed View 的区别：

| 类型 | row callback | 更新已有 key 时 | 适用场景 |
| --- | --- | --- | --- |
| `RenderOnlyKeyedListView` | `Fn(T, usize) -> AnyView` | 重新渲染 row content，但按 key 保持顺序和 row range | 内容由 item/index 完全决定 |
| `StatefulKeyedListView` | `Fn(T, usize, RowUpdater<T>) -> AnyView` | 通过 updater 更新 row 内部 state | 需要保留 row 内部 View、listener 或 DOM identity |

stateful row 的 callback 必须在首次 render 中调用 `updater.bind(...)`；未绑定会
使 initial render 失败。`RowUpdater::bind` 每个 generation 只能成功一次；
`update(item, index)` 在 callback 执行期间暂时取出 callback，因此重入 update 会
返回错误。row 被移除后 updater generation 失效，旧 updater 的 update 不会再触发
新的 row。

```rust
let list = StatefulKeyedListView::new(
    values,
    Rc::new(|value: &i32| *value),
    Rc::new(|value: i32, index, updater| {
        updater.bind(|next, next_index| {
            // 用 next / next_index 更新 row 自己的状态。
            let _ = (next, next_index);
            Ok(())
        });
        AnyView::from(Element::with_child("li", format!("{index}:{value}")))
    }),
    None,
);
```

片段中的 `bind` 返回 `bool`；真实代码应检查它是否为 `true`，并确保 callback
确实负责更新 row content。`error_handler` 参数为 `Option<ErrorHandlerToken>`；
为 `None` 时沿用父 context 的 handler。

一次 keyed 更新先读取 source、计算全部 key，并在状态修改前捕获 key function
panic、拒绝重复 key。这个准备阶段的错误不会进入 reconcile rollback，也不会修改
旧 row state。key 校验通过后，框架才进入 row update、新 row render 和 range move
阶段；这一阶段的返回错误或 panic 才会触发 rollback，尝试恢复旧顺序、已更新 row
的旧 item snapshot，并清理 pending row。恢复本身失败时，会把恢复错误合并成
fatal framework error。成功完成 reconcile 后，删除的 row 才会关闭；删除 cleanup
失败是提交后的 close error，不属于前述旧状态恢复路径。

当前实现还把查找 `range.end` 的 parent 操作放在上述 `result` rollback 区域之外；
正常 mounted range 不会触发该路径，但若 backend 在此处返回错误，相关 state 恢复
没有独立测试覆盖。

## 自动响应式 View

`AutoReactiveView` 为基础文本类型（如 `String`、数字、`bool`、`char`）提供
响应式文本节点实现；`Element`、`AnyView`、`Option`、`ViewCons` 和 typed element
则通过 dynamic View 重新挂载。因而 `Signal<T>`、`ReadSignal<T>`、`Computed<T>`
和 `StoredValue<T>` 可以直接作为 View：

```rust
let label = context.access().signal(String::from("before"))?;
context.mount_unit(label, handler.view())?;
label.set(String::from("after"))?;
```

文本 source 更新时复用同一个 text node，只更新其文本；复合 View source 更新时
使用动态 range 替换其 content。source 的生命周期受创建它的 owner scope 约束，
owner 关闭后再读取 source 会返回相应 reactive error。

## 共同不变量

动态 renderer、branch 和 list 都通过 `MountTarget::before` 在 anchor end 前插入
内容。业务代码不要手动把 row 节点移出 range、删除 anchor 或跨 context 移动
content；这会破坏 row invariant，使后续 reconcile 返回 framework error。每个
row 的 `AnyView` 可以产生零个、一个或多个节点，连续范围会把它们作为整体清理
和移动。

实现与测试入口：`src/flow/dynamic.rs`、`branch.rs`、`indexed.rs`、`keyed.rs`、
`rows/block.rs`、`rows/updater.rs`，以及 `tests/kernel.rs` 和 `tests/browser.rs`
中的 dynamic、branch、indexed/keyed、multi-node row 与 rollback 测试。
