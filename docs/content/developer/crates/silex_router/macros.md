+++
title = "router 宏与类型生成"
description = "silex_macros::router 的输入语法、生成 enum、编译期校验和嵌套布局。"
weight = 50
+++

# router 宏与类型生成

`router!` 实际实现位于 `crates/silex_macros/src/route.rs`。宏先解析一个
route tree，再为每个 enum 生成 variant、路径编码、静态 pattern 描述、已编译
typed matcher 和 route table 构造。它的职责是把“路由值”与“URL pattern”绑定，
运行时 matcher 仍由 `silex_router::route_table` 执行。

## 输入形状

叶子 route 使用 `=>`：

```rust
router! {
    pub enum AppRoute {
        Home => "/",
        User { id: u32 } => "/users/:id",
        Files { rest: PathTail } => "/files/*rest",
        NotFound => "/*",
    }
}
```

带命名字段的叶子 variant 必须用 struct-like body；字段名必须覆盖 pattern
中的每个命名 `:parameter` 和 `*wildcard`，不能多写、少写或重复。字段类型
在 `path()` 时必须实现 `PathParam`；wildcard 字段还必须精确写成
`PathTail`（允许通过路径导入，不要求限定 crate 名）。

嵌套路由引用另一个独立生成的 enum：

```rust
router! {
    enum AppRoute {
        Users(UsersRoute) {
            prefix: "/users";
            layout: |_ctx, outlet| outlet;
        },
    }
}
```

`UsersRoute` 必须在输入位置可解析为类型。nested body 只接受 `prefix` 和
`layout`；children 不能内联在同一个 body 中。layout closure 必须恰好接收
两个输入：`RouterContext` 和 child `AnyView` outlet。

## 生成的 API

宏为 enum 生成以下 API。`AppRoute` 对应的 typed matcher 类型名为
`AppRouteMatcher`，可见性与 `AppRoute` 相同：

| API | 结果 |
| --- | --- |
| `route.path()` | `Result<RoutePath, PathParamError>`；将 variant 字段编码成 pathname。 |
| `Route::patterns()` | `&'static [&'static str]`；只返回当前 enum 层的静态 pattern 描述，不执行编译。 |
| `Route::compile()` | `Result<RouteMatcherType, RoutePatternError>`；初始化当前层和所有 nested child 的 compiled matcher。 |
| `RouteMatcherType::match_path(path)` | `Result<Option<Route>, PathError>`；使用已保存 matcher 按候选解析 enum。 |
| `Route::table(render)` | `Result<RouteTable<'scope>, RoutePatternError>`；生成 exhaustive view handler 表。 |

生成 enum 和上述关联 API 的 visibility 与输入 enum 相同。宏不自动生成
`Clone`、`Debug` 或 `PartialEq`；如果应用需要这些 trait，应在外围设计中另
行维护，不能假定宏会提供。

typed matching 应在初始化边界编译一次，并在后续路径循环中借用对象：

```rust
let routes = AppRoute::compile()?;
for path in paths {
    if let Some(route) = routes.match_path(path)? {
        handle(route);
    }
}
```

只需要原始 `RouteMatch` 时，不要为了类型转换构造 table；使用同一份静态描述
创建并保存 `RouteMatcher`：

```rust
let matcher = RouteMatcher::from_patterns(AppRoute::patterns())?;
let matches = matcher.matches(path)?;
```

这两个对象都应放在 setup、应用状态或请求处理器上下文中，而不是 signal effect、
导航 handler 或每次 pathname 变化的闭包中。

`table` 的 render closure 类型是
`Fn(Route, RouterContext<'scope>) -> AnyView<'scope>`。它不是 `FnOnce`，且闭包
及返回 view 受 `'scope` 限制；这保证 route handler 捕获的 signal、context
和 owner-bound view 不会逃逸。对于组件，先用组件宏生成符合 scope 的 view，
再在 render closure 中调用其 builder。

## 编译期校验

宏侧会在生成代码前拒绝：

- variant 名重复；
- pattern 不以 `/` 开头、含 query/fragment、含空 segment 或非法 percent encoding；
- 参数名不符合 ASCII 标识符规则；
- 参数名重复或 wildcard 不在末尾；
- 字段数量、字段名称与命名参数不一致；
- wildcard 字段不是 `PathTail`；
- 两个 route 的 decoded pattern shape 重复；
- nested prefix 含动态参数，或其合成 `prefix/*` 与同 enum 的 pattern 冲突；
- nested layout 不是两个参数。

这些错误发生在编译期，适合使用 trybuild 锁定诊断。运行时仍可能发生
`PathParamError`：例如 typed 值的 `encode_segment` 失败，或 `RoutePath` 的
最终验证失败；调用者必须处理 `path()` 的 `Result`。

## fallback 与 nested 生成

typed matcher 和生成的 table handler 都先将 `RouteMatch` 解析为 enum。若 `u32` 等字段无法从
raw segment 解码，该 handler 返回 `None`，`RouteTable` 会继续尝试后面的
matcher 候选。因此把 `NotFound => "/*"` 放在 enum 中可以承接 typed route
失败和未命中的路径。

nested variant 的 `path()` 先调用 child `path()`，再通过
`join_route_paths` 合并静态 prefix；typed matcher 先匹配合成的 `prefix/*`，
再用 `strip_route_prefix` 得到 child relative path，并调用 compile 阶段已经
保存的 child matcher。`table()` 则递归构造 child table，再用 nested layout
调用 `RouteTable::nest`。

## 维护与性能边界

宏代码在编译期有一份 pattern parser，运行时 `RouteMatcher` 有另一份 parser；
两者必须保持规范化、参数名和 wildcard 规则一致。修改其中一份时，应同时
更新 `tests/routes_macro.rs`、UI pass/fail fixtures 和 path/table 单元测试。

`compile()` 的 `RoutePatternError` 属于初始化错误；对象 `match_path()` 和
`RouteMatcher::matches()` 只在路径输入阶段报告 `PathError`。对象匹配不会
重新调用 `RouteMatcher::from_patterns` 或 `add_pattern`。当前没有已验证的
benchmark 数字，文档不对延迟、吞吐或复杂度作承诺。

## 相关源码与测试

- `crates/silex_macros/src/route.rs`：解析、校验和 token generation。
- `crates/silex_router/tests/routes_macro.rs`：生成 path、match、table 和 nesting。
- `crates/silex_router/tests/ui/pass_routes_macro*.rs`：合法 API、组件和多层 nested route。
- `crates/silex_router/tests/ui/fail_router_macro*.rs`：字段、参数、wildcard、重复和 prefix 诊断。
- `crates/silex_router/tests/compile_fail.rs`：trybuild 总入口。
