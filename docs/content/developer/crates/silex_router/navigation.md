+++
title = "浏览器导航与链接"
description = "silex_router 的 RouterContext、Navigator、Router、RouteOutlet、Link 和 owner 清理。"
weight = 40
+++

# 浏览器导航与链接

浏览器侧 router 把 URL 拆成两个响应式来源：`path` 保存相对于 base 的
pathname，`search` 保存原始 query string。`RouterContext` 同时提供
`Navigator` 和由 `search` 派生的 `query_map`，让组件在不直接访问
`window.location` 的情况下完成导航和查询响应。

## `RouterContext`

`RouterContext<'scope>` 是 `Copy` 的 scope-bound 能力值，公开字段包括：

| 字段/API | 语义 |
| --- | --- |
| `base_path` | owner 中保存的规范化 base，例如 `/app`。 |
| `path` | 相对 base 的 pathname signal；根路径为 `/`。 |
| `search` | `?` 开头的原始 query string；没有 query 时为空字符串。 |
| `navigator` | 当前 context 的导航控制器。 |
| `query_map()` | 从 `search` 派生的 `Rx<HashMap<String, String>>`。 |
| `owner()` / `error_reporter()` | 将组件逻辑绑定到相同 owner 和错误报告边界。 |

`RouterContext::new` 适合测试或自定义集成：调用方需要提供同一 runtime 中
的 `ReadSignal<String>`、对应的 `WriteSignal<String>` 和
`SilexContext`。构造函数会先验证四个 signal/write destination，再创建
query computed；runtime 不一致会返回 `SilexError`，而不是延迟到第一次
导航才失败。

query parser 使用浏览器 `UrlSearchParams`：`?empty` 和 `?empty=` 都得到
空字符串，重复 key 在 `HashMap` 中由后出现的 value 覆盖。`query_map()` 是
computed，因此 `search` 更新会触发依赖它的 effect/view。

## `Navigator`

`Navigator` 的路径 API 接受实现 `ToRoute` 的值：`&str`、`String`、
`&String` 和 `RoutePath` 已内置实现。推荐 typed route 使用生成的
`Enum::path()?` 或 `RoutePath::new(...)`，因为 raw string 实现不会自动执行
`RoutePath` 的 pathname 验证。

| 方法 | 行为 |
| --- | --- |
| `push(to)` | 调用 `history.pushState`，产生新的 history entry，然后刷新 path/search。 |
| `replace(to)` | 调用 `history.replaceState`，复用当前 history entry，然后刷新状态。 |
| `set_query(key, Some(value))` | 修改/添加 key，并以 push 导航到当前 pathname。 |
| `set_query(key, None)` | 删除 key；若序列化结果没有变化则不产生导航。 |
| `refresh_location()` | 读取浏览器 pathname/search，剥离 base，并以去重方式更新 signal。 |

`push`/`replace` 构造 History API URL 时，如果 logical path 以 `/` 开头，
会把规范化 base 加在前面；以 `/app` 为 base 时，逻辑 `/users` 对应浏览器
URL `/app/users`。带 `?` 的字符串会作为完整 logical URL 传给 History API，
但 query 不会进入 `path` signal。

导航、History API、`UrlSearchParams` 或 signal 更新失败都会返回
`SilexResult<()>`。组件事件 handler 应把它返回给 `silex_dom` 的错误边界，
而不是静默忽略。

## `Router` 的挂载顺序

```text
Router(ctx).base(...).routes(table).layout(...).build()
                         │
                         ▼
                   create RouterContext
                         │
                         ▼
                mount RouteOutlet + layout
                         │
                         ├─ register owner-bound popstate
                         └─ resolve current path
```

`Router` 是 `silex_macros::component` 生成的 builder facade。`routes` 是
required chain prop；`base` 默认 `/`，`layout` 默认不包裹 outlet。挂载时：

1. 检查 `RouterView` 保存的 context 创建结果；
2. 在传入 `MountOwner` 下注册 `popstate`，回调调用 navigator 刷新位置；
3. 创建 `RouteOutlet`，再把它交给可选 layout；
4. mount layout/outlet。若 listener 或 view mount 失败，会返回错误；不会在
   listener 注册失败后继续调用 route handler。

`window` 不存在时 context 创建会保存 fatal JavaScript error，后续 mount 返回
该错误。因此 native 代码可以编译 route table、path 和 macro，但不能把
浏览器 `Router` 当作 native DOM 入口调用。

## `RouteOutlet` 与 nested outlet

outlet 是一个 `View`，而不是一次性渲染函数。它订阅 `RouterContext::path`，
对每个稳定 branch 使用 `mount_branch_stable_cached`：同一 branch key 保留
branch owner，key 变化时关闭旧 branch 并挂载新 view。nested table 的 outlet
先剥离静态 prefix，再把相对 path 交给 child table。

这解释了两个常见行为：

- `/users/1` 到 `/users/2` 会替换包含 `id` 的叶子 view；
- 如果 `/users` 是 nested layout，父 layout 不因 child id 变化而重新创建，
  只有 child outlet 的分支变化。

没有匹配时 outlet 返回空 view。要显示 404，应在 table 中注册显式 fallback。

## `Link` 的点击边界

`Link(ctx, to)` 生成 HTML `<a>`，并将逻辑路径与 base 拼成显示用 `href`。
设置 `active_class` 后，active 状态是响应式的：除根路径外，精确路径和
segment 边界下的子路径都算 active，例如 `/users` 匹配 `/users/42`，不匹配
`/username`。

只有同时满足以下条件时，Link 才调用 `prevent_default` 并使用
`navigator.push`：

- primary button（button 0）；
- 没有 ctrl/meta/shift/alt 修饰键；
- `target` 为空且 `download` 为 false；
- href 是同源内部路径。

中键、新标签页修饰键、下载、显式 target、外部 origin 和非标准 href 保留
浏览器默认行为。这是可访问性和安全边界的一部分；不要通过自定义点击逻辑
强行拦截所有 anchor 行为。

## owner 清理与错误

`RouterView` 注册的 listener 使用传入的 `MountOwnerToken`。owner close 会
移除 listener、关闭动态 outlet 的 branch owner，并清除旧 route view。路由
handler、layout 和点击 handler 持有的 `Rc`/signal 也因此受 scope 约束；不能
将 `RouterContext`、`AnyView` 或依赖它们的闭包提升为 `'static`。

修改导航或 outlet 时，应检查 `crates/silex_dom` 的 owner 清理契约：
listener 失败要能返回、mount 失败不能留下 outlet、owner close 后迟到的
popstate 不能执行用户代码、旧 branch cleanup 不能重复运行。

## 相关源码与测试

- `crates/silex_router/src/context.rs`：context、query computed、base 剥离和 navigator。
- `crates/silex_router/src/lib.rs`：Router component、RouterView 和 RouteOutlet。
- `crates/silex_router/src/link.rs`：Link、active class 和点击拦截条件。
- `crates/silex_router/tests/router.rs`：browser navigation、layout、query、listener 和 cleanup。
- `crates/silex_router/tests/context_inputs.rs`：runtime mismatch 的早期拒绝。
