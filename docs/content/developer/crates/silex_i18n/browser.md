+++
title = "浏览器、持久化与 Intl"
description = "silex_i18n 的浏览器 locale detection、document metadata、locale binding 和平台格式化。"
weight = 30
+++

# 浏览器、持久化与 Intl

浏览器相关能力都通过 feature 显式启用。`browser` 负责读取
`navigator.languages`、判断文字方向和同步文档 metadata；`intl` 在此基础上
调用 JavaScript `Intl`。这些 API 仍由 `I18nStore` 的 owner 管理，不会建立全局
locale singleton。

## 浏览器 locale 选择

```rust
let selected = detect_browser_locale(&available, &fallback);
```

`navigator_languages` 先读取 `navigator.languages` 中能被 `Locale` 解析的值；
如果结果为空，再尝试 `navigator.language`。无效浏览器字符串会被忽略。

`resolve_requested_locale` 的顺序是：

1. requested locale 与 available locale 完全相等；
2. requested 与 available 的 language 相等；
3. requested 的 fallback chain（跳过完整值）中存在 exact available；
4. 返回调用方提供的 fallback。

它不会对 available 做排序，也不会返回“最接近”的新 Locale；结果始终是
available 中已有的 clone，或 fallback 的 clone。

`locale_direction` 对 `ar`、`dv`、`fa`、`he`、`ku`、`ps`、`ur`、`yi` 返回
`TextDirection::Rtl`，其它语言返回 `Ltr`。这是文档方向的约定集合，不是完整
Unicode bidi 分析器。

## 同步 `<html lang>` 与 `dir`

```rust
let effect = store.sync_document_metadata()?;
```

在 wasm browser 中，这会在 document element 上维护一个由 owner 管理的 metadata
记录，并随 store.locale 更新 `lang` 与 `dir`。多个嵌套 store 同时同步时，内部
stack 保持 owner 顺序：只有当前栈顶记录直接控制属性；关闭顶层 owner 后，会在
未被外部修改的前提下恢复上一层记录的值。

这意味着该 API 会尊重外部 DOM 修改：如果应用或其它脚本已经把属性改成与本记录
最后写入值不同的内容，cleanup 不会无条件覆盖它。`EffectHandle` 可以显式 stop，
但 owner close 仍是 metadata record 和 effect 的最终清理边界。

没有 document 时（包括 native 测试），crate 会创建后立即停止的 inactive effect，
不分配实际响应式节点或 cleanup；调用方可以安全地保留返回的 handle，但不应把
native 行为当作浏览器 metadata 已生效。

## locale 持久化和路由输入

`persist` feature 下，locale binding 可以来自 `silex_persist` 的 local/session
storage、自定义 backend 或 `silex_router` query backend。`I18nBuilder` 只要求
拿到 `Persistent<Locale>`；它会校验 binding source 与当前 owner 属于同一 runtime，
并建立 store ↔ binding 的双向 effect。

持久化同步的顺序是：

```text
backend / query event
        │
        ▼
Persistent<Locale> ──► I18nStore.locale()
        ▲                     │
        └──── set_locale ◄────┘
```

相等 locale 不会再次写 backend。owner close 会释放 subscription、effect 和
backend 绑定；跨 tab 冲突、敏感数据保护和 query 的重复 key 语义仍由
`silex_persist`/`silex_router` 决定。

## `intl` feature

`Intl::number(locale)`/`NumberFormat::format` 处理有限 `f64`；非有限值返回
`IntlErrorKind::InvalidValue`。`Intl::date_time(locale)` 和
`DateTimeFormat::format` 接受 Unix timestamp milliseconds，非有限值同样报错。
也可以使用根级 `format_number`、`format_date_time` 函数。

wasm 目标通过 JavaScript 的 `Intl.NumberFormat`/`Intl.DateTimeFormat` 构造器
格式化；构造器缺失、JS throw 或返回值不是字符串时会转换为 recoverable
`IntlErrorKind::JavaScript`。native 目标使用 crate 内置的轻量 fallback：数字只
覆盖源码列出的分组/小数分隔符，日期固定输出 `YYYY-MM-DD HH:MM:SS UTC`。

因此 Intl 输出是平台边界，不要在 native 单元测试中断言浏览器的完整 locale
格式，也不要把 native fallback 当成 ICU/CLDR 的完整实现。需要用户可见的精确
格式时，应在目标浏览器上做 wasm 测试。

## 安全、错误与性能边界

- 浏览器 `lang`/`dir` 的值来自经过 `Locale` 校验的字符串，但翻译文本本身仍
  可能来自外部 catalog；不要把翻译结果拼入 HTML/脚本上下文。
- `navigator.languages` 的无效值会被静默跳过；如果最终返回 fallback，应在产品
  层决定是否需要记录诊断，而不是假设浏览器一定提供合法 locale。
- Web Storage 是同步 API，`Persistent` 的立即写入策略可能出现在响应式更新
  路径。频繁切换 locale 时应结合 `silex_persist` 的写入策略评估，不对延迟作
  未经基准验证的承诺。
- `sync_document_metadata` 维护的是属性所有权记录，不是 document 的全局锁；
  外部脚本与多个 owner 同时修改时，以“最后写入且仍被本记录控制”为清理条件。

## 对应源码与测试

- `crates/silex_i18n/src/browser.rs`：locale detection、direction 和 metadata stack。
- `crates/silex_i18n/src/intl.rs`：native fallback、wasm JavaScript Intl adapter。
- `crates/silex_i18n/tests/browser.rs`：storage、query、metadata、DOM translation 和 browser locale。
- `crates/silex_i18n/tests/wasm.rs`：resource、suspense 和异步 owner cleanup。
- `docs/content/developer/crates/silex_persist/`：backend 与写入策略的完整契约。
- `docs/content/developer/crates/silex_router/`：query signal 与浏览器导航的完整契约。
