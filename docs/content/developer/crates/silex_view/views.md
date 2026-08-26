+++
title = "View、元素与类型擦除"
description = "说明 silex_view 的 View 契约、元素 builder、Tag metadata 和 AnyView。"
weight = 10
+++

# View、元素与类型擦除

`View<'scope>` 是 `silex_view` 的最小渲染契约：实现者提供
`mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>>`。
View 不直接持有 document，也不返回 browser 节点；它通过 context 的
`MountTarget` 写入由调用方注入的 `DomContext`。

## `Element` 与 `TypedElement`

`Element::new(tag)` 创建 HTML namespace 的未类型化元素，`Element::new_svg(tag)`
创建 SVG namespace 的元素；`with_child` 是创建元素并加入一个 child 的便捷构造。
child 必须实现 `View<'scope>`，字符串、数字、布尔值、`Option`、`Vec` 等值会通过
`AnyView` 的 `From` 实现转成文本、空 View 或子列表。

```rust
let view = Element::with_child(
    "section",
    vec![
        Element::with_child("h1", "Title"),
        Element::with_child("p", "Body"),
    ],
);
```

上面的代码是 API 片段；它省略了 `MountContext` 和 `SilexResult` 所在的外层
函数，不是 `docs/examples/` 中的独立 CI 示例。

`TypedElement<'scope, T>` 额外携带 `T: Tag` marker。`Tag::METADATA` 提供 tag 名称、
namespace 和 void 标记，`from_tag`/`with_child_from_tag` 据此创建元素。marker
只参与 Rust 类型能力和 metadata，不包含 `web_sys` 类型。

```rust
#[derive(Clone, Copy)]
struct SvgRect;

impl Tag for SvgRect {
    const METADATA: TagMetadata =
        TagMetadata::new("rect", TagNamespace::Svg, false);
}

let rect = TypedElement::<SvgRect>::with_child_from_tag("square");
```

仓库中的 codegen 可以通过 `define_tag!` 生成 tag marker 和 builder。手工实现
`Tag` 时，`TagMetadata::new` 的 `namespace` 与 `is_void` 必须反映实际元素；SSR
serializer 会根据这些 metadata 选择 namespace 和是否输出闭合标签。

`TypedElement::into_untyped` 可以去掉 marker；`Element` 与 `TypedElement` 都实现
`AttributeBuilder` 和 `View`，因此属性链可以在类型转换前后使用。

## `AnyView` 与组合 View

`AnyView<'scope>` 是 owner-bound 的类型擦除容器，公开变体为：

| 变体 | 语义 |
| --- | --- |
| `Empty` | 不产生节点；`()`、`None` 和 `ViewNil` 会映射到此变体。 |
| `Text(String)` | 产生一个文本节点；字符串和 primitive `From` 会映射到此变体。 |
| `Element(Element)` | 保存未类型化元素。 |
| `List(Vec<AnyView>)` | 按顺序挂载多个 child。 |
| `Boxed(Rc<dyn View>)` | 保存任意自定义 View 或闭包生成的 View。 |

`View::into_any` 和 `AnyView::new` 要求 View 至少活到当前 `'scope`；`Rc` 让
`AnyView` 可 clone，但不会复制已经挂载的 DOM。`AnyView` 也实现了从 `Vec<V>`、
`Option<V>` 和 `ViewCons<H, T>` 的转换，适合在动态 renderer 或 list row 中统一
返回类型。

```rust
let optional: AnyView<'_> = Some(Element::with_child("span", "shown")).into();
let children: AnyView<'_> = vec![
    Element::with_child("span", "one"),
    Element::with_child("span", "two"),
].into();
```

## `MountInstance` 不负责清理

`MountContext::mount` 返回 `MountInstance`，其中保存本次调用产生的
`DomNode` 快照；可用 `nodes`、`first_node`、`len`、`is_empty` 或 `into_nodes` 读取
这些句柄。元素和文本 View 通常会填充节点，但 `AnyView::Empty`、`AnyView::List`
以及 `ViewCons` 等 composite View 也可能返回空的 `MountInstance`，因为它们把
清理和子节点挂载交给当前 owner/context。`MountInstance` 带有 `'scope` marker，
但不拥有 owner，也不会在 drop 时自动移除节点。View 实现必须把 effect、listener、
NodeRef binding 和节点移除注册到 `MountOwnerToken`，而不是把 `MountInstance` 当作
dispose handle。

元素 View 的内部流程是：创建元素 -> 插入当前 `MountTarget` -> 合并并应用属性
和事件 -> 在 child context 中挂载 children -> 向父 owner 注册逆序 cleanup。child
失败时会关闭 provisional owner 并移除已经插入的元素，因此自定义 View 也应遵守
同样的 rollback 语义。

## 自定义 View 的约束

自定义 View 应使用传入的 `MountContext`，通过 `context.dom()` 和
`context.target()` 执行 backend-neutral 操作；不要自行创建全局 DOM context。需要
挂载子 View 时通过借用调用 `context.mount(&child)`，需要把工作延迟到提交后时调用
`context.on_commit`。返回的每个节点都应属于当前 target 的同一 backend context，
否则会得到 `CrossContext` 或 parent/reference 错误。

响应式 View、动态闭包和列表的 owner/anchor 约束见[动态 View、分支与列表](flow.md)。
应用级 host、staging 和 dispose 见[挂载、提交与清理](mounting.md)。
