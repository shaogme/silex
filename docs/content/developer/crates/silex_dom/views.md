+++
title = "视图、动态分支与列表"
description = "silex_dom 的 View 契约、类型擦除、响应式视图、动态分支和列表 diff。"
weight = 30
+++

# 视图、动态分支与列表

`silex_dom::view` 把“要创建什么 DOM”与“由哪个 owner 管理它”分开。
`View<'scope>` 只描述一次可重复的 mount；真实 Node、effect、cleanup 和
宿主资源在传入 `MountOwner` 下创建。这个设计使同一个 view factory 可以
用于多个独立挂载，也使动态替换时旧子树可以先关闭再移除。

## `View` 与 `MountInstance`

`View` 的核心方法是：

```rust
fn mount(
    &self,
    owner: &dyn MountOwner<'scope>,
    parent: &web_sys::Node,
    attrs: Vec<AttrOp<'scope>>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<MountInstance<'scope>>
```

自定义 view 实现应遵守三个契约：

1. 失败时返回 `SilexError`，不要把失败写到日志后伪造成功；
2. 已经追加的节点和已注册的资源必须由本次子 owner cleanup 或失败回滚
   清除；
3. 不要保存已经挂载的 DOM 句柄来实现“复用”。`View` 是工厂，复用的是
   描述和闭包；每次 `mount` 应返回新的 `MountInstance`。

`MountInstance` 是节点快照：`nodes()`、`first_node()`、`len()`、
`is_empty()` 和 `into_nodes()` 只让调用方观察或转移本次产生的 Node。它
不拥有 owner，也不会因为 `MountInstance` 被 drop 就代替 scope cleanup。
类型标记为 `#[must_use]`，因为忽略一次 mount 结果很容易把 DOM 句柄和错误
路径丢掉。

## 基础视图与组合

crate 内置以下 `View` 实现：

| 形态 | 结果 |
| --- | --- |
| `String`、`&str`、`Cow<str>`、数字、`bool`、`char` | 创建一个 text node。 |
| `()`、`ViewNil`、`Option::None` | 不创建节点。 |
| `Option<V>` | `Some` 挂载内部 view，`None` 为空视图。 |
| `Vec<V>`、数组 | 通过 composite owner 和 fragment 挂载多个子 view。 |
| `ViewCons<H, T>` / `chain!` | 用嵌套 cons 组合多个异构 view。 |
| `SilexResult<V>` | `Ok` 挂载内部 view，`Err` 原样传播。 |
| `AnyView` | 将上述形态或自定义 `View` 做 owner-bound 类型擦除。 |
| `Fn() -> V` | 每次动态 render 时创建一个新的 `V`。 |

复合 view 的顶层 `attrs` 转发给第一个子项（即使它是空视图）；之后的
sibling 使用空属性列表。需要让一个具体元素承担 class、style 或事件时，应把属性直接
加在该元素上，或者显式构造一个 wrapper，而不要依赖一个空 sibling 接收
属性。

`AnyView::new(view)` 用 `Rc<dyn View<'scope> + 'scope>` 保存 view factory，
因此可以在 `Vec<AnyView>`、动态分支和列表 factory 中统一返回不同元素。
`view_match!` 把 match 的各分支转换为 `AnyView`；它只做类型擦除，不会
改变 branch owner 或错误语义。

## 元素与 typed tag

`Element::new("div")` 构造通用 HTML 元素，`Element::new_svg("svg")` 使用
SVG namespace。`Element::with_child(tag, child)` 直接追加一个初始 child。
`TypedElement<'scope, T>` 额外携带 `T: Tag` marker，`Tag::DomElement`
描述目标 `web_sys` 类型，trait marker（`FormTag`、`TextTag`、`SvgTag` 等）
供上层属性 facade 做能力约束。

`define_tag!` 有两种常用形式：

```rust
silex_dom::define_tag!(
    Icon,
    web_sys::SvgElement,
    "svg",
    icon,
    new_svg,
    non_void,
    [SvgTag, TextTag]
);

let view = icon("diagram");
```

`non_void` 生成接收 child 的函数，`void` 生成无 child 参数的函数。HTML
完整标签集合在 `silex_html` 中由同一宏生成；DOM crate 不隐式定义这些
应用层标签。

## 响应式视图

实现 `AutoReactiveView` 的类型可以直接由 `silex_core::Rx` 驱动。文本
类型（`String`、`&str`、`Cow<str>`、数字和 bool）会创建一个文本节点，
首次 effect 写入当前值，后续 tracked 读取变化只更新该节点；元素、
`AnyView`、`Option<V>` 和 `ViewCons` 则把当前值当作动态 view，在自己的
范围内替换 DOM。

```rust
let (label, set_label) = owner.signal("first")?;
let view = label.into_rx();
context.mount(view, error_handler)?;
set_label.set("second")?;
```

片段说明了数据流，但 `owner`、`context` 和 `error_handler` 依赖外层
`MountContext`，不是独立文档示例。实际使用时，`Rx` 必须与 mount target
属于兼容的 runtime；读取错误交给传入的 error handler，owner close 后旧
view 的 effect 不会继续写入 DOM。

底层动态 kernel 使用 comment anchor 维护一个 range。第一次 render 失败
时移除 range；后续 render 失败时保留前一次成功内容，并将错误交给
handler，这样一次暂时的业务/DOM 错误不会自动把已提交内容替换成半成品。

## 稳定 branch

`mount_branch_stable_cached` 让 branch evaluation 返回
`BranchEvaluation<K, S>`：只比较 `K` 决定 identity，`S` 是新 row 首次
挂载时使用的 snapshot。key 相同不会因为 snapshot 改变而重新 mount，key
改变才会替换 branch 内容。

`BranchRenderContext` 同时暴露：

- `owner()`：该 branch 的持久 `OwnerAccess`，适合 page/runtime 状态；
- `content_owner()`：该次 DOM 内容的 `MountOwnerToken`，负责节点和内容
  cleanup；
- `error_handler()`：branch-safe handler view。

两种 owner 分离是有意的：结构性路由/branch effect 可以保留，而 branch
内容替换时只关闭旧 content owner。不要把 content owner 的 DOM cleanup
注册到 branch runtime owner，也不要把 `BranchEvaluation::snapshot` 当成
用于比较 identity 的字段。

## 列表实现与 identity

三个公开列表类型针对不同的更新契约：

| 类型 | identity | row factory | 适用场景 |
| --- | --- | --- | --- |
| `IndexedListView` | index/位置 | `Fn(T, usize) -> AnyView` | 行 identity 随位置变化的简单列表。 |
| `RenderOnlyKeyedListView` | `key_fn(&T)` | `Fn(T, usize) -> AnyView` | key 保持 row identity，但每次 item/index 变化都重渲染。 |
| `StatefulKeyedListView` | `key_fn(&T)` | `Fn(T, usize, RowUpdater<T>) -> AnyView` | key 相同就保留 row owner 和局部状态。 |

列表 source 必须实现 `RxRead`、`ReactiveSource` 和 `ForLoopSource` 契约，
因此可以是 `Vec<T>`、`Option<Vec<T>>` 或 `SilexResult<Vec<T>>` 等 core
支持的输入。keyed list 的 key 必须 `Hash + Eq + Clone`；初始或更新时
出现重复 key 会返回 framework error，并回滚尚未提交的 rows。

`RowUpdater<T>` 是 stateful row 的可选 typed capability：

- `bind` 只能为当前 generation 绑定一次 callback；
- `update(item, index)` 在 active row 上调用 callback 并返回 `true`；
- row 被删除、key generation 失效或 callback 未绑定时返回 `false`；
- callback panic 会被恢复后重新抛出，不能把 panic 当作普通更新失败。

不要把旧 row 的 updater 保存成应用级全局 setter。generation 失效后它会
变成 inert，这正是防止删除后的 row 继续写入新 DOM 的保护。

## 失败与 owner 回滚

复合 view 使用 provisional child owner 和 fragment。任一 child mount 返回
错误时，先关闭 provisional owner，再把 fragment 中的节点留在 detached
状态或移除；如果 provisional cleanup 自身失败，主错误仍保留，但关闭
错误会通过 `MountOwner` 的 reporter 单独报告。列表更新同样先准备新 rows，
失败时恢复旧 row snapshot 并 dispose pending rows。

维护自定义 view 时，最容易破坏的不是“能否创建一个 div”，而是失败路径：
要覆盖 partial DOM、cleanup error、重复 branch replacement 和 stale
updater。对应行为见 `tests/owner.rs` 和 `tests/mounted_app.rs`。
