+++
title = "节点树与渲染"
description = "silex_dom 的节点模型、树操作、范围移动和 SSR 序列化。"
weight = 10
+++

# 节点树与渲染

`silex_dom` 用 `DomNode` 表示任意 backend-neutral 节点，用
`DomElement`、`DomDocument` 提供更窄的能力边界。节点不暴露 `web_sys::Node`
或 SSR 内部记录；所有树操作都必须经过创建它的 `DomContext`。

本文的短片段用于说明真实 API 的调用关系，省略了外层函数和错误处理上下文；
它们不是 `docs/examples/` 中由 CI 编译的独立示例。完整流程见总览页的
`docs/examples/silex_dom/ssr.rs`。

## 节点模型

`NodeKind` 有五种值：`Document`、`Element`、`Text`、`Comment` 和 `Fragment`。
`DomContext::element(&node)` 会同时检查 backend identity 和 node kind，只有
元素节点才会返回 `DomElement`。
`DomElement` 和 `DomDocument` 不暴露具体 backend 对象；需要回到通用树 API 时，
分别通过 `node()` 取得 `&DomNode`。

`ElementSpec::new(name)` 创建 HTML namespace 的元素，并且只有当 `name` 恰好是
小写 HTML void 名称时才设置 `is_void()`；`ElementSpec::namespaced(name, namespace, void)` 用于
SVG、MathML 或自定义 namespace，并要求调用方显式提供 void 标记。

```rust
let main = context.create_element(ElementSpec::new("main"))?;
let svg = context.create_element(ElementSpec::namespaced(
    "svg",
    Namespace::Svg,
    false,
))?;
let text = context.create_text("hello")?;
let comment = context.create_comment("marker")?;
let fragment = context.create_fragment()?;
```

`Namespace::Html.uri()` 返回 `None`；`Svg`、`MathMl` 和 `Custom` 返回对应 URI。
browser backend 据此选择 `create_element` 或 `create_element_ns`，SSR serializer
据此在 namespace 切换处输出 `xmlns`。

## 插入、移动与删除

`append` 是 `insert_before` 的无 reference 形式。需要精确位置时使用
`InsertRequest::before(parent, node, reference)`：如果 node 已经在树中，backend
会先将它从原 parent 移除，再插入 reference 前；reference 必须是目标 parent 的
子节点。把同一 node 作为 node 和 reference 是 no-op。

fragment 插入会按原顺序移动 fragment 的全部子节点，而不会把 fragment 本身
作为输出树中的可见元素。SSR backend 会拒绝 document 作为普通 child、cycle、
把 child 放进 text/comment，以及向 void element 插入子节点。browser backend
会先验证所属 `Document` 和句柄，再委托原生 `appendChild`/`insertBefore`；原生
DOM 产生的异常会包装成 `DomError::Backend`，并且当前没有用 `is_void()` 主动
拦截向 void element 追加子节点。

```rust
context.append(parent.node(), &child)?;
context.insert_before(InsertRequest::before(
    parent.node(),
    &child,
    reference.node(),
))?;
context.remove(&child)?;
```

`remove` 要求节点已有 parent；移除 document、移除 detached node 或使用不同
context 的 node 都会返回 `DomError`。SSR 会在 mutation 前显式检查这些条件；
browser 会通过原生 DOM 的 parent/remove 结果返回 `NoParent` 或包装后的 backend
错误。因此不要依赖两个 backend 为每种非法树操作返回同一个具体 variant。

## 连续范围

`DomContext::range(RangeRequest)` 创建同一 parent 下从 `start` 到 `end` 的闭区间。
构造时会确认两个边界都是 parent 的 child，且 start 不在 end 之后。
`DomRange::nodes()` 返回当前区间快照；`remove()` 逐个删除节点，
`move_before(target_parent, reference)` 则把整段作为一次 backend range operation
移动到 reference 前。

```rust
let range = context.range(RangeRequest {
    parent: parent.node().clone(),
    start: start.clone(),
    end: end.clone(),
})?;
range.move_before(target.node(), reference.node())?;
```

reference 不能位于待移动区间内，否则返回 `DomError::ParentMismatch`。调用方不要
在 range 创建后假设节点位置永远不变；`nodes()` 会根据当前 parent 重新查找边界。

## SSR 序列化

启用 `ssr` 后，`SsrDom::new()` 创建 document 和内存树。调用
`SsrDom::serialize(SerializeOptions::default())` 序列化 document，或使用
`serialize_node` 序列化某个节点。默认输出 comment；设置
`SerializeOptions { include_comments: false }` 可以省略 comment。

serializer 会对 text、attribute、namespace URI 和 comment 做转义；comment 中
连续的 `--` 会被改写，以避免生成非法 comment。HTML void element 只输出开始标签，
普通元素输出成对标签；fragment 和 document 只输出其子节点。SSR 用 `BTreeMap`
保存 attribute，因此同一状态下属性输出顺序稳定；这不是 browser DOM 的顺序契约。

SSR backend 保存的是逻辑树，不代表浏览器中已连接的节点。SSR 的
`context.document_body()` 返回 `None`，`focus` 等 browser-only 能力返回
`DomError::Unsupported`。

## backend 差异与审阅提示

`ElementSpec::is_void()` 会影响 SSR parent 校验和 serializer；当前 browser tree
操作没有在 `create_element`/`append` 路径复用这个标记来拒绝向 void element
追加子节点，因此跨 backend 使用 void element 时不要依赖 browser 接受该操作，
也不要把 browser 行为当作 SSR contract。若修改这一边界，应同时补 browser 与
SSR 测试。

`DomContext::range` 只保证 start/end 是同一 parent 的闭区间。调用
`move_before` 时，target parent 不应位于待移动 range 的后代树中；当前 browser
和 SSR 的 range move 实现没有把这个情形作为单独的公开错误变体，调用方应避免
提交这种结构，backend 后续也应补充一致性校验。

## 相关边界

- 属性和 property 的写入规则见[属性与 property](attributes.md)。
- listener 不属于树 mutation；它返回的取消 lease 见[事件与宿主资源](events.md)。
- `DomNode` 的 context identity、detached 和 wrong-kind 错误见[错误模型](errors.md)。
- 上层 View 把这些 primitive 组合为 mount transaction，见[View 与 mount 的上层边界](views.md)。

对应实现和测试：`src/runtime/context.rs`、`src/runtime/range.rs`、
`src/runtime/tree.rs`、`src/adapters/ssr/tree.rs`、`src/adapters/ssr/serialize.rs`、
`tests/ssr/tree.rs`。
