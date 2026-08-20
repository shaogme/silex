+++
title = "silex_rx"
description = "Silex rx! 响应式表达式过程宏的语法、展开边界和验证方法。"
template = "section.html"
sort_by = "weight"
+++

# `silex_rx`

`silex_rx` 是 Silex 的 `rx` 过程宏 crate。它把带有 `$` 响应式源标记的 Rust 表达式转换为 `silex_core` 的 owner-owned `Rx`、常量或 `Callback`，让组件可以用接近普通 Rust 表达式的写法声明派生值。它不保存 runtime，也不实现响应式图；真正的 owner、promotion、依赖追踪、错误 handler 和清理都由 `silex_core` 完成。

## 在 Silex 架构中的位置

```text
组件 / store / 业务表达式
          │
          ▼
       rx! 宏
  源标记解析 · 依赖读取包装
          │  生成调用
          ▼
     silex_core facade
  SilexContextProvider · OwnerAccess
  promote · computed_always · constant · callback
          │
          ▼
  silex_reactivity runtime
```

应用通常使用 `silex_core::rx!`，它会把当前 crate 的 `$crate` 路径和 `@ctx` 语法转发给本 crate 的过程宏。直接使用过程宏主要适合 facade、宏测试和需要显式控制依赖前缀的场景。

## 稳定入口与输出

| 入口 | 作用 | 返回形态 |
| --- | --- | --- |
| `silex_core::rx!(ctx; body)` | 应用层推荐入口；自动使用 `silex_core` facade。 | 通常是 `SilexResult<Rx<'scope, T>>`；参数化闭包是 `SilexResult<Callback<'scope, T>>`。 |
| `silex_rx::rx!(silex_core; @ctx ctx; body)` | 过程宏的完整调用形式；第一个片段是生成代码所用的 facade 路径。 | 与上行相同。 |
| 空 body | 为当前 owner 创建 `()` 常量。 | `SilexResult<Rx<'scope, ()>>`。 |

`rx!` 生成的代码会通过 `SilexContextProvider::owner` 获取 owner，通过 `SilexContextProvider::error_reporter` 获取 `ErrorReporter<'scope>`。生成代码内部使用 `?`，但这些 `?` 位于宏生成的闭包中；宏表达式本身返回 `SilexResult`，不会把错误传播到调用函数。调用方可以使用 `?`、`match` 或其它 `Result` 处理方式决定如何处理 promotion、初始计算和 callback 创建错误。

## 最小可运行流程

下面的源码位于 `docs/examples/silex_rx/basic.rs`，页面直接读取该文件，不在 Markdown 中维护第二份示例。它同时演示过程宏的完整形式、`silex_core::rx!` facade 和显式字段源。

{% set source = load_data(path="examples/silex_rx/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

示例由 `crates/tests/silex_macros_test/tests/docs_examples.rs` 编译。示例本身只用 `?` 传播错误，没有用 `unwrap` 或 `expect` 隐藏响应式 API 的错误路径。

## 源标记

### shorthand 源

`$name` 把一个标识符注册为响应式源。宏会在生成代码中用该源的 `with` 读取包住剩余表达式，所以它沿用该值已有的 tracked 读取语义：

```rust
let doubled = silex_core::rx!(ctx; $count * 2)?;
```

这里的 `count` 必须是可作为 `ReactiveSource` 使用的值；`$count` 不是字符串替换，也不会创建同名变量。

### 显式字段源

`$(settings.theme)` 把字段本身注册为响应式源，适合 `#[store]` 生成的字段或包含响应式字段的局部结构：

```rust
let label = silex_core::rx!(ctx; format!("Theme: {}", $(settings.theme)))?;
```

`$(...)` 内部只接受路径、字段访问和外层括号。下面的写法会在宏展开阶段报错，应把方法调用放在源标记外：

```rust
// 错误：显式源不能包含方法调用
let value = silex_core::rx!(ctx; $(settings.theme.clone()))?;

// 正确：先读取字段源，再在表达式中调用方法
let value = silex_core::rx!(ctx; $(settings.theme).clone())?;
```

相同语法和相同 token 文本的源只注册一次；`$state` 与 `$(state)` 属于不同语法形式，不应依赖二者被合并。

## 表达式分类

宏根据 body 的语法选择 owner 构造器：

| body 形态 | 生成的 core API | 语义 |
| --- | --- | --- |
| 以字面量开始且没有响应式源，例如 `42` | `OwnerAccess::constant` | owner 管理一个非依赖图的常量值。 |
| 普通表达式，例如 `$count * 2` | `OwnerAccess::computed_always` | 创建会重新求值的派生值；源读取错误交给 reporter。 |
| 无参数闭包，例如 `\|\| $count * 2` | `OwnerAccess::computed_always` | 将闭包作为派生计算体，并强制 `move` 捕获。 |
| 有参数闭包，例如 `\|event\| ...` | `OwnerAccess::callback` | 创建 scope-owned callback；调用时才执行 callback body。 |
| 以 `@fn` 开头，例如 `@fn 42` | `OwnerAccess::computed_always` | 禁用字面量常量快捷路径，强制走 computed 构造。 |

普通表达式和无参数闭包的成功值会转换为 `Rx`，参数化闭包的成功值会转换为 `Callback`，外层统一包装为 `SilexResult`。过程宏生成的每个可能失败的 owner 操作都在宏内部使用 `?`；调用方不必位于 `Result` 函数中，也可以先保存或匹配返回的 `SilexResult`。

## 生命周期与并发边界

- 宏只使用传入 context 的 owner，不创建全局 runtime、线程局部 runtime 或隐式 owner。
- 生成的 `Rx`、`Callback` 和它们捕获的响应式源都绑定到 context 的 owner lifetime；owner 关闭后，继续使用句柄会遵循 `silex_core` 的 stale-node 错误语义。
- 过程宏不会把 `Rc`、`RefCell` 或 owner capability 变成 `Send`/`Sync`；并发和跨 runtime 规则仍由 `silex_core` 与 `silex_reactivity` 执行。
- 代码生成阶段没有 `unsafe`。宏生成的 callback/derived closure 使用 `move` 捕获，以满足 owner scope 的存活约束；不要把生成结果保存到比 context 更长的生命周期。
- `SilexContextProvider` 要求 context 实现 `Clone + Copy`，并提供 `owner`、`error_reporter` 和 `with_error_reporter`。过程宏使用前两个能力；自定义 context 仍须完整实现该 trait。

## 源码与测试索引

- 过程宏解析、源注册、token 重写和展开：`crates/silex_rx/src/lib.rs`
- `silex_core::rx!` facade 与 `SilexContextProvider`：`crates/silex_core/src/lib.rs`、`crates/silex_core/src/context.rs`
- 响应式源 promotion 和 `Rx` 构造：`crates/silex_core/src/owner.rs`、`crates/silex_core/src/reactivity/`
- token 级单元测试：`crates/silex_rx/src/lib.rs` 的 `#[cfg(test)] mod tests`
- 宏实际编译契约：`crates/tests/silex_macros_test/tests/macro_ui.rs` 与 `tests/ui/pass_macro_*.rs`
- 可执行文档示例：`docs/examples/silex_rx/basic.rs`
- 文档示例编译测试：`crates/tests/silex_macros_test/tests/docs_examples.rs`

## 专题

- [过程宏展开流程与维护边界](expansion.md)：输入解析、源注册、visitor 重写和四类构造路径。
- [测试与调试](testing.md)：token 单元测试、trybuild 编译契约和文档示例验证。
- [silex_core 响应式值与派生](@/developer/crates/silex_core/reactivity.md)：生成代码实际调用的 `Rx`、promotion、tracked 读取和宏 facade。
- [silex_core owner 生命周期](@/developer/crates/silex_core/lifecycle.md)：context、scope、handler 与清理语义。

## Feature 与已知限制

`crates/silex_rx/Cargo.toml` 没有声明 feature，也没有运行时依赖；`syn`、`quote` 和 `proc-macro2` 只用于编译期 token 处理。

维护时需要特别注意以下限制：

- 直接调用 `silex_rx::rx!` 时，第一个分号前的 token 会被拼接到 `prefix::SilexContextProvider`、`prefix::ErrorReporter` 等路径；它必须是可用的 `silex_core` facade 路径。应用层优先使用 `silex_core::rx!`，以避免手写 prefix。
- 宏只识别 `$name` 和 `$(path.field)`；索引、方法调用、函数调用、算术表达式或任意复杂表达式不能放进 `$(...)`。
- 生成的 computed 使用 `computed_always`，不要把 `rx!` 生成的派生值误认为 equality-gated `computed`。需要不同通知策略时应直接调用 `OwnerAccess` API。
- 参数化闭包是 callback，不是自动订阅的 computed；它只在 `Callback::invoke`/`call` 时执行。callback 调用错误仍须由调用方处理。
- token 文本重写会递归进入嵌套宏调用，因此新增 marker 语法或改变 visitor 时，必须同时检查字符串格式化等嵌套宏场景。
