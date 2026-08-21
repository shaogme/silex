+++
title = "路径、参数与编码"
description = "silex_router 的 pathname 规范化、类型化参数、wildcard 和 percent 编码契约。"
weight = 20
+++

# 路径、参数与编码

`silex_router::path` 将浏览器 pathname 拆成可验证的 segment，并在静态
pattern、typed 参数和 wildcard 之间复用同一套 percent codec。这里的
`RoutePath` 表示已经验证的本地路径；它不是包含 query 或 fragment 的完整
URL。

## 三种路径输入

| 类型 | 作用 | 失败方式 |
| --- | --- | --- |
| route pattern | 声明静态段、`:parameter` 或末尾 `*wildcard` | `RoutePatternError` 或宏编译错误 |
| `RoutePath` | 传递给导航或链接的已验证本地 pathname | `PathError` |
| raw `&str`/`String` | 通过 `ToRoute` 传给导航 API | 由 History API 或下游使用处报告错误；不会自动做 `RoutePath` 验证 |

## 规范化规则

`normalize_path`、`RoutePath::new` 和 matcher 的 pattern parser 共享这些
规则：

- 空字符串和 `/` 规范化为 `/`；末尾一个 slash 是可选的并会被去掉，根路径
  仍保持 `/`。
- 路径必须以 `/` 开头；中间的空 segment（例如 `/users//42`）会被拒绝，
  不会被静默合并。
- `?` 和 `#` 不属于 route path。`RoutePath::new("/users?tab=all")`
  会返回 `PathError`；查询参数应使用 `Navigator::set_query`。
- 每个 segment 的 percent encoding 都会被检查，解码结果必须是合法 UTF-8。
  普通静态字符不会在规范化时被解码，因此路径显示仍保留原始编码。

`join_route_paths(prefix, suffix)` 要求两侧都是以 `/` 开头的合法路径，在根
路径和非根路径之间正确合并。`strip_route_prefix` 按解码后的 segment 比较，
但返回剩余部分的原始 encoding；因此 `/a%2Fb` 只匹配同一个编码为一个
segment 的前缀，不会匹配 `/a/b`。

## pattern 语法

| 形式 | 捕获范围 | 生成 enum 字段 |
| --- | --- | --- |
| `/users` | 精确静态 segment | 无 |
| `/users/:id` | 一个 segment，保留原始 raw 后再按目标类型解码 | 字段名必须是 `id` |
| `/files/*rest` | 从当前位置到 pathname 末尾的零个或多个 segment | 字段类型必须是 `PathTail` |
| `/*` | 从当前位置到末尾，不产生命名参数 | 无；只适合作为 fallback |

参数名的首字符必须是 ASCII 字母或 `_`，其余字符必须是 ASCII 字母、数字
或 `_`。wildcard 必须是最后一个 segment；同一个 pattern 不能重复参数名。
同一张 matcher/table 中，静态值解码后相同、参数形状相同的 pattern 也不能
重复，例如 `/:id` 与 `/:name` 会冲突。

## `PathParam` 与 percent codec

`PathParam` 要求实现 `decode_segment` 和 `encode_segment`，并且错误类型必须
实现 `Error + Into<SilexError> + 'static`。crate 已实现以下类型：

- `String`：percent 解码后保留任意 UTF-8 字符；编码时 slash、query 保留字、
  空格和 `%` 等都被编码。
- `bool`、全部有符号/无符号整数、`f32`、`f64`：先 percent 解码，再调用
  类型自身的 `FromStr`。
- `char`：解码后必须恰好包含一个 Unicode scalar value。
- `PathTail`：按 raw slash 拆分后逐 segment 解码，重新编码时保留 segment
  边界。于是 `a%2Fb/c` 的逻辑值是 `a/b/c`，但再次编码仍是 `a%2Fb/c`。

`percent_encode_segment` 只原样保留 RFC 3986 unreserved 字符
`A-Z a-z 0-9 - . _ ~`；其它 UTF-8 字节都使用大写十六进制 `%XX`。它不会
把 `+` 当作空格。`percent_decode_segment` 对不完整的 `%`、非十六进制字节
和无效 UTF-8 返回可恢复的 `PathError`。

自定义参数类型可以实现 `PathParam`，但必须把 segment 边界当作安全不变量：
编码结果不能包含未编码的 `/`、`?` 或 `#`，否则 `RoutePathBuilder` 最终验证
时会失败。不要把已经 percent 编码的字符串再次当作已解码值传入，否则 `%`
会被再次编码。

## 生成路径与解析路径

由 `router!` 生成的 `path()` 使用 `RoutePathBuilder`：静态段先校验，typed
字段调用 `PathParam::encode_segment`，最后由 `RoutePath::new` 做整体验证。
因此调用方应处理 `Result<RoutePath, PathParamError>`，不要用字符串拼接代替
typed path。

生成的 `RouteEnum::compile()` 返回 typed matcher 和
`RoutePatternError`；生成 matcher 的 `match_path()` 返回
`Result<Option<RouteEnum>, PathError>`：

- 非法 pathname 返回 `Err`；
- 没有候选或所有候选都无法把参数解析为字段类型时返回 `Ok(None)`；
- typed 参数解析失败只跳过当前候选，后续 wildcard 等候选仍有机会匹配。

应用应在 setup 边界保存编译结果：

```rust
let routes = RouteEnum::compile()?;
let route = routes.match_path(path)?;
```

`RouteEnum::patterns()` 只提供静态 pattern 描述；raw matching 应通过一次性
创建并保存的 `RouteMatcher::from_patterns(RouteEnum::patterns())` 完成。因而
pattern 配置错误属于 compile/setup 阶段的 `RoutePatternError`，pathname 验证
错误仍属于匹配阶段的 `PathError`。

匿名 `/*` 可以被 matcher 匹配，但生成的 enum `path()` 没有可供编码的值，
会返回 recoverable 的 `PathParamError`。这正是 fallback route 应该只用于
解析和渲染、不要用于生成链接的原因。

## 相关源码与测试

- `crates/silex_router/src/path.rs`：路径类型、codec、builder 和 prefix 工具。
- `crates/silex_router/src/route_table.rs`：pattern parser、参数捕获和匹配。
- `crates/silex_macros/src/route.rs`：宏侧 pattern parser 与字段校验。
- `crates/silex_router/tests/routes_macro.rs`：typed path、decode 和 fallback。
- `crates/silex_router/src/path.rs` 的测试：编码、slash 边界、非法输入和 nesting prefix。
