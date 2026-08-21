+++
title = "匹配表与嵌套路由"
description = "silex_router 的 RouteMatcher、RouteEntry、RouteTable、匹配优先级和嵌套 outlet。"
weight = 30
+++

# 匹配表与嵌套路由

`RouteMatcher` 只负责把 pathname 变成有序的 `RouteMatch`；`RouteTable` 在此
基础上附加 scope-bound view handler。将匹配与渲染分开，可以在 native 测试
中验证路径规则，也可以让 `RouteOutlet` 只在 branch 发生变化时替换 view。
对渲染路径而言，`RouteTable` 是 render-time reuse boundary：在合法
`RouterContext<'scope>` 的 setup 边界构造一次，随后把同一张 table 交给
`Router`/`RouteOutlet`。

## `RouteMatcher` 的数据流

```text
pattern ── parse/normalize ──► compiled segments + shape key
                                      │
                                      ▼
pathname ── split raw segments ──► static ──► param ──► wildcard
                                      │
                                      ▼
                                Vec<RouteMatch>
```

`RouteMatcher::from_patterns` 和 `add_pattern` 在编译阶段拒绝非法 pattern、
重复参数和重复 shape。`matches(path)` 会验证 pathname，再返回所有候选；
`match_path(path)` 只取第一个；`resolve(path, handler)` 按顺序调用 handler，
直到某个 handler 返回 `Some`。

`RouteTable::matcher()` 只读取 table 已保存的 compiled matcher，不会重新解析
entry pattern。纯 pathname 代码应显式保存一个 `RouteMatcher`；需要 scope-bound
handler 和 view 的代码应保存 `RouteTable`，不要在同一次请求或每次导航中另建
一张 raw matcher。

匹配优先级由 matcher tree 的遍历顺序决定：同一位置先尝试 decoded static
child，再尝试单段 parameter，最后尝试 wildcard。测试已验证
`/files/new` 的顺序是精确静态、`:id`、`*rest`、根 wildcard。顺序不等于
“第一个注册的 pattern 永远优先”；pattern 必须先通过 shape 去重，handler
仍可以返回 `None` 让后续候选继续处理。

## 读取 `RouteMatch`

`RouteMatch<'path>` 借用传给 matcher 的 pathname，公开以下信息：

| API | 值 |
| --- | --- |
| `path()` | 原始 pathname，包括保留的 percent encoding。 |
| `route_id()` | 加入 matcher 的零基 route position。 |
| `params()` | 按 pattern 顺序排列的 `RouteParam`。 |
| `param(name)` / `raw(name)` | 读取命名参数和其 raw segment。 |
| `parse::<T>(name)` | 通过 `PathParam` 解码并转换为目标类型。 |

`RouteParam::parse` 和 `RouteMatch::parse` 返回
`RouteMatchError<T::Error>`。缺少名字是 `Missing`；编码或类型转换失败是
`Decode`。handler 不应假设参数一定能解析为业务类型：返回 `None` 可以让
fallback route 接管，而把错误直接传播到业务 view 则适合需要显示错误的
场景。

## `RouteEntry` 与 `RouteTable`

手动创建渲染表的最小形状如下（签名来自 `RouteEntry::new`，这里只说明 API
结构）：

```rust
RouteEntry::new("/users/:id", |matched, ctx| {
    let id = matched.parse::<u32>("id").ok()?;
    Some(render_user(id, ctx))
})?
```

这个片段依赖调用方的 scope、`render_user` 和 view 类型，不能独立编译；完整
的可编译示例见总文档的 `docs/examples/silex_router/basic.rs`。

`RouteEntry<'scope>` 保存 pattern 和
`for<'path> Fn(RouteMatch<'path>, RouterContext<'scope>) -> Option<AnyView<'scope>>`
handler。`RouteTable::from_entries` 会在 setup 时把所有 entry 编译进 matcher，
因此返回错误必须在初始化阶段处理，尤其是重复 shape。table 可以 clone，因为
entry handler 内部由 `Rc` 保存，但它仍然受 `'scope` 限制；clone 不应被当作跨
runtime 或跨线程的全局缓存。

`RouteTable` 的常用入口：

- `matcher()`：读取底层 compiled matcher，适合检查 route count、pattern 或
  只做 raw matching；
- `matches` / `match_path`：只匹配，不创建 view；
- `resolve(path, ctx)`：运行第一个返回 `Some(AnyView)` 的 handler；
- `nest(prefix, child, layout)`：把 child table 放在静态 prefix 下，并在
  prefix branch 中渲染 layout 与 child outlet。

## 嵌套 prefix 与 layout

`nest` 只接受静态 prefix，例如 `/users`。实现会把它注册成合成的
`/users/*` branch，并将 child 的相对 pattern 与 prefix 组合：

```text
parent: /app/*
child:  /users/*
leaf:   /:id

browser pathname: /app/users/42
parent branch:    /app/*
child relative:   /users/42
leaf relative:    /42
```

prefix 中不能出现 `:tenant` 或 `*rest`。nest 会拒绝以下冲突：

- parent leaf 与 child root 组合后相同；
- child wildcard 与 parent 合成 branch 相同；
- sibling nested prefix 产生相同的 shape。

layout 回调接收 `RouterContext` 和 child `AnyView` outlet，返回任意实现
`View<'scope>` 的值。回调由 `RouteTable` 转成 `AnyView`，因此可以写静态
外壳、导航栏或错误区域；child route 变化时，父 nested branch 的 key 不含
具体 child 参数，layout 可以保持挂载。

## router 宏如何构造 table

`router!` 生成的 `Enum::table(render)` 应在 render setup 边界调用一次，会：

1. 为叶子 variant 创建 `RouteEntry`；
2. 在 handler 中把 `RouteMatch` 解析为 enum variant；typed decode 失败时
   返回 `None`，因此可回退到后续 pattern；
3. 为 nested variant 递归调用 child enum 的 `table`，再调用 `nest`；
4. 把用户 layout 的结果转换为 `AnyView`。

这让 render closure 可以对生成的 enum 做 exhaustive `match`，同时保留
`RouterContext` 的 scope。路由表构造失败是 `RoutePatternError`，不应该用
`unwrap` 隐藏配置冲突。

## 与 `RouteOutlet` 的关系

`Router` 为根 table 创建 `RouteOutlet`。outlet 读取 context 的 path signal，
根据当前 pathname 生成 branch key，然后调用 `RouteTable::resolve_branch`。
path evaluation 和 branch render 都读取已经保存的 table；它们不会在 signal
变化时重新调用 `Enum::table`、`RouteTable::from_entries` 或 pattern parser。
每个稳定 branch 会得到独立的 DOM branch owner；当前 branch 失效时，旧 view
先清理，再挂载新 view。无匹配路径返回空 `AnyView`，因此应用若需要 404，
应显式添加末尾 wildcard entry。

## 相关源码与测试

- `crates/silex_router/src/route_table.rs`：matcher tree、entries、nesting 和 branch key。
- `crates/silex_router/src/lib.rs`：`RouteOutlet` 与 branch render。
- `crates/silex_macros/src/route.rs`：生成 enum table 与 nested layout 的代码。
- `crates/silex_router/tests/routes_macro.rs`：宏生成 table 和 exhaustive render。
- `crates/silex_router/src/route_table.rs` 的测试：优先级、fallback、nest 冲突和多层 prefix。
