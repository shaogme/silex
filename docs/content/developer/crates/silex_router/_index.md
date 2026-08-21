+++
title = "silex_router"
description = "Silex 的类型化路径、路由匹配、嵌套 outlet 与浏览器导航运行时。"
template = "section.html"
sort_by = "weight"
+++

# `silex_router`

`silex_router` 把 URL pathname 映射为可渲染的 Silex view，并把浏览器的
History API、`popstate`、查询参数和链接行为接入 `silex_core` 的 owner 与
`silex_dom` 的 view 生命周期。它既提供不依赖浏览器对象的路径和 matcher，
也提供浏览器端的 `Router`、`Navigator`、`Link` 和动态 outlet。

## 在 Silex 架构中的位置

```text
router! / RouteEntry
          │
          ▼
 RouteMatcher ──► RouteTable ──► RouteOutlet / AnyView
       │              │                 │
       │              └─ RouterContext ──┘
       │                         │
       ▼                         ▼
  path codec              silex_core owner/signals
                                    │
                                    ▼
                         silex_dom view / host listener
```

路径模块不需要 `window` 或 `document`，适合 native 编译、服务端准备工作和
单元测试。`Router` 构造的 context 会读取 `window.location`，挂载时还会注册
owner-bound 的 `popstate` listener，因此真正的路由 view 需要浏览器环境。

## 稳定入口与核心类型

| 入口 | 作用 |
| --- | --- |
| `router!` | 从 route enum 声明生成 enum、`path`、`match_path` 和 `table`。 |
| `RoutePath` | 已验证的本地 pathname；拒绝 query、fragment、空路径段和无效 percent encoding。 |
| `PathParam` / `PathTail` | 定义单段参数的编解码；`PathTail` 表示最终 wildcard 捕获的多段值。 |
| `RouteMatcher` | 编译静态、动态和 wildcard 模式，返回按优先级排列的 `RouteMatch`。 |
| `RouteEntry` / `RouteTable` | 将 matcher 模式与 scope-bound view handler 组合，并支持静态前缀嵌套。 |
| `RouterContext` | 暴露 `base_path`、pathname、search、`Navigator` 和响应式 query map。 |
| `Navigator` | 通过 `pushState`、`replaceState` 和 `popstate` 更新逻辑路径。 |
| `Link` | 输出 `<a>`，只拦截普通的同源主按钮点击，其余行为交给浏览器。 |
| `Router` / `RouteOutlet` | 注册导航监听、创建动态 outlet，并把匹配的 handler 挂载为 view。 |

应用通常从 `silex_router` 根导入这些类型；需要明确依赖边界时，可以从
`path`、`route_table`、`context` 和 `link` 模块逐项导入。`core`、`dom`、
`macros` 和 `reexports` 模块是给生成代码与组件示例使用的 facade。

## 生命周期与并发边界

一次已挂载的 router view 的主要 owner 关系如下：

```text
SilexContext<'scope>
└── RouterContext<'scope>
    ├── path/search signals + query computed
    ├── owner-bound popstate listener
    └── RouteOutlet
        └── stable branch owner
            ├── route handler view
            └── nested RouteOutlet / child branch
```

- `RouterContext<'scope>`、`RouteTable<'scope>`、`AnyView<'scope>` 和布局闭包
  都携带调用方 scope。它们不能保存到 `'static` 或跨线程使用；内部使用
  `Rc`，与 `silex_core`/`silex_dom` 一样是单线程模型。
- `RouterContext::new` 会在创建 query computed 之前验证 pathname、search、
  两个写 signal 与当前 owner 属于同一个 runtime。手动构造 context 时，
  所有输入必须来自同一 runtime。
- `Router` 挂载时把 `popstate` listener 注册到传入的 `MountOwner`。owner
  关闭会移除 listener、关闭 outlet 分支并清理当前 route view；关闭后到达的
  `popstate` 不会再次渲染路由。
- outlet 的分支 key 由匹配的 route id 和原始参数组成。普通参数变化会替换
  当前叶子分支；嵌套前缀使用无参数的合成分支，所以父 layout 可以保持不变，
  只让 child outlet 更新。
- `RouteMatch<'path>` 借用传入的 pathname；handler 如果要读取参数，应在
  handler 的调用期间使用 `raw` 或 `parse`，不要把 `RouteMatch` 脱离该路径
  的生命周期保存。

## 最小可运行流程

下面的源文件同时验证路径生成、percent 编码、typed match、失败后的
wildcard fallback 和 route table 构造。它不挂载浏览器 DOM，因此 native
测试即可覆盖；页面直接读取这一个文件，不在 Markdown 中维护第二份 Rust
代码。

{% set source = load_data(path="examples/silex_router/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

浏览器应用在获得 `SilexContext` 后，将生成的 table 交给 `Router`：
`Router(ctx).base("/app").routes(table).build()`。`routes` 是 required
chain prop；`base` 默认是 `/`，`layout` 默认直接显示 outlet。这个调用会
在 mount 时读取当前 pathname/search，并由 `Router` 的 owner 管理导航监听。

## Feature、平台与外部边界

`crates/silex_router/Cargo.toml` 没有声明 router 自身的 feature flag。它依赖
`silex_core` 的 `error-router`，依赖 `silex_dom`/`silex_html` 提供 view 与
`<a>`，并通过 `web_sys` 使用 `Window`、`Location`、`History`、
`UrlSearchParams` 和 `MouseEvent`。

公开边界包括以下几类不变量：

- `RoutePath` 与 path codec 只接受本地 pathname；query string 和 fragment
  必须通过 `Navigator::set_query` 或显式 URL 字符串处理，不能混入 route
  pattern。
- percent encoding 在 segment 边界内解码。`a%2Fb` 是一个值为 `a/b` 的
  segment，不等于两个 pathname segment；wildcard 则逐段解码并保留边界。
- `RouterContext` 的 query map 是 `HashMap<String, String>`，重复 key 会
  保留 `UrlSearchParams` 遍历到的最后一个 value，不提供重复值列表或顺序。
- `Router` 和 `Link` 的 JavaScript cast、History API 和 DOM listener 错误
  都通过 `SilexResult`/传入的 error handler 报告；文档示例不使用
  `unwrap`/`expect` 掩盖这些错误。

## 专题

- [路径、参数与编码](paths.md)：`RoutePath`、`PathParam`、wildcard、规范化和 percent codec。
- [匹配表与嵌套路由](tables.md)：`RouteMatcher`、`RouteEntry`、`RouteTable`、优先级和 outlet 分支。
- [浏览器导航与链接](navigation.md)：`RouterContext`、`Navigator`、`Router`、`RouteOutlet` 和 `Link`。
- [router 宏与类型生成](macros.md)：`router!` 输入语法、编译期校验和生成 API。
- [测试与调试](testing.md)：native、trybuild、browser 和文档示例的验证边界。

## 源码、示例与测试索引

- facade 与 Router view：`crates/silex_router/src/lib.rs`
- context、query computed 和 navigator：`crates/silex_router/src/context.rs`
- `Link` 与点击拦截：`crates/silex_router/src/link.rs`
- path codec、`RoutePath` 和 prefix 工具：`crates/silex_router/src/path.rs`
- matcher、entry、table 和 nesting：`crates/silex_router/src/route_table.rs`
- `router!` 解析与代码生成：`crates/silex_macros/src/route.rs`
- 文档示例：`docs/examples/silex_router/basic.rs`
- 文档示例测试：`crates/silex_router/tests/docs_examples.rs`
- native 路径、matcher 和 table 测试：`src/path.rs`、`src/context.rs`、
  `src/link.rs`、`src/route_table.rs` 的 `#[cfg(test)]` 模块以及
  `tests/routes_macro.rs`、`tests/context_inputs.rs`
- 编译期契约：`tests/compile_fail.rs` 和 `tests/ui/`
- 浏览器生命周期与 DOM 行为：`tests/router.rs`

## 已知限制与维护注意

- `Router` 使用 History API，不提供服务器端 fallback。部署应用时，深层
  pathname 需要由宿主服务器返回应用入口；否则浏览器刷新不会回到 Silex
  router。
- `Router::base` 只规范化前导/末尾 slash，并按 pathname segment 边界剥离
  base。`/app` 不会错误地匹配 `/application`；导航到逻辑 `/` 时浏览器 URL
  会是 `/app/`。
- 直接传入 `&str`/`String` 会走 `ToRoute`，不会自动构造 `RoutePath`。对由
  用户输入或外部数据产生的本地路径，优先先调用 `RoutePath::new`，再传给
  `Navigator`/`Link`；query 应通过 `set_query` 管理。`Link` 只拦截普通同源
  主按钮点击，外部链接、下载、修饰键和非默认 target 保留浏览器语义。
- `router!` 生成的 `match_path` 每次调用都会重新构造 `RouteMatcher`。没有
  基准数据时不对延迟或复杂度作数字承诺；高频原始匹配应复用手动创建的
  `RouteMatcher`，渲染场景则复用 `RouteTable`。
- `Link` 的 `active_class` 只比较逻辑 pathname，不比较 query；`/users`
  会匹配 `/users/42`，但不会匹配 `/username`。需要 query 级高亮时，应在
  组件中读取 `RouterContext::query_map` 自己组合条件。
- 修改 route branch key、owner 关闭顺序或 listener 安装时，必须同时验证：
  父 layout 是否在 child navigation 中保持、旧 view 是否只清理一次、owner
  关闭后的 listener 是否不会触发用户 handler，以及错误是否仍交给正确的
  reporter。

验证本 crate 文档或公开 API 变更时，至少运行目标 crate 的 `cargo check`、
相关测试和 `zola --root docs check`。新增或修改 `docs/examples/silex_router/`
后，优先运行 `cargo test -p silex_router --test docs_examples`，再按环境追加
browser runner；不需要为了该示例运行整个 workspace 的测试。
