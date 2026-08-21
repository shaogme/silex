+++
title = "silex_i18n"
description = "绑定 Silex owner 的 locale、catalog、翻译 memo、复数和浏览器国际化能力。"
template = "section.html"
sort_by = "weight"
+++

# `silex_i18n`

`silex_i18n` 将 locale、翻译 catalog 和格式化辅助能力接入
`silex_core` 的 owner 与响应式运行时。它解决的是“当前作用域应该显示哪一条
消息，以及 locale 或 catalog 改变后如何自动更新”的问题；它不负责从网络获取
文件、不负责把字符串解析成 HTML，也不提供跨线程的全局翻译状态。

在 Silex 架构中，它位于应用/DOM/路由与 `silex_core` 之间。`silex_router` 或
`silex_persist` 可以作为 locale 的输入来源，但所有带 scope 的句柄仍由创建它们
的 owner 管理。

## 在 Silex 架构中的位置

```text
组件 / DOM / Router / 持久化 locale
              │
              ▼
      I18nBuilder ──► I18nStore
      Locale · Catalog · t!
              │
              ├── CatalogResource / Resource
              ├── browser metadata / Intl
              └── optional Persistent binding
              │
              ▼
        silex_core owner/runtime
```

## 稳定入口与核心类型

| 入口 | 主要用途 |
| --- | --- |
| `Locale` | 校验、规范化 locale，并生成从具体到语言级的 fallback chain。 |
| `Catalog` / `CatalogValue` / `Message` | 保存一个 locale 的文本或复数消息；支持运行时构造，`json` feature 还支持 JSON。 |
| `PluralForms` / `PluralCategory` | 定义复数类别，所有复数消息都必须有 `other`。 |
| `I18nBuilder` | 绑定 `OwnerAccess` 和 `ErrorReporter`，配置 locale、catalog 和缺失策略。 |
| `I18nStore` | 读取/修改 locale、fallback locale 和 catalog，并执行同步翻译。 |
| `t!` / `Argument` | 创建跟踪 locale、catalog revision 和参数 source 的翻译 `Rx<String>`。 |
| `CatalogResource` | 按当前 locale 异步加载 catalog，并复用 store cache。 |
| `I18nVariant` / `I18nKeys` | 将 enum 变成类型化翻译输入；后者由可选 `macros` feature 提供。 |
| `detect_browser_locale` / `sync_document_metadata` | `browser` feature 下读取浏览器偏好并同步 `<html lang/dir>`。 |
| `Intl` / `NumberFormat` / `DateTimeFormat` | `intl` feature 下执行数字和 Unix 毫秒时间戳格式化。 |

应用若只使用运行时能力，通常直接从 `silex_i18n` 根导入。`I18nKeys` 只有在
启用 `macros` feature 后才会从根重新导出；也可以直接依赖
`silex_i18n_macros`。

## Feature 与平台边界

`silex_i18n` 默认不启用 feature：locale、内存 catalog、同步翻译和
`CatalogResource` 的类型可以在 native 构建。可选 feature 如下：

| Feature | 内容 | 额外边界 |
| --- | --- | --- |
| `json` | `Catalog::from_json`，将嵌套对象展平为点号 key。 | 依赖 `serde_json`；输入必须是字符串或合法复数对象。 |
| `persist` | `Persistent` 重导出与 `I18nBuilder::locale_binding`。 | locale binding 必须和 i18n store 属于同一个 runtime。 |
| `browser` | 浏览器 locale、语言方向和 document metadata。 | 需要 `wasm32`/`web_sys`；native 下 metadata effect 为空且立即停止。 |
| `intl` | `Intl`、数字和日期时间格式化。 | 同时启用 `browser`；wasm 使用 JavaScript `Intl`。 |
| `macros` | 从 `silex_i18n_macros` 重导出 `I18nKeys`。 | 派生宏在编译期读取 JSON catalog。 |
| `browser-tests` | crate 浏览器集成测试所需的依赖组合。 | 主要用于仓库测试，不是应用功能开关。 |

若通过顶层 `silex` facade 使用，分别对应 `i18n`、`i18n-json`、
`i18n-persist`、`i18n-browser`、`i18n-intl` 和 `i18n-macros` feature；
这些 facade feature 的定义以 `crates/silex/Cargo.toml` 为准。

## 生命周期与并发边界

```text
Runtime
└── OwnerAccess<'scope>
    ├── I18nStore<'scope>（Copy 的能力句柄）
    │   ├── locale/fallback signals
    │   ├── catalog registry + revision
    │   └── translation computed / CatalogResource
    └── error handler、cleanup、scoped futures
            │ owner close
            ▼
       失效句柄、取消 loader、移除 browser metadata
```

- `I18nStore`、翻译 `Rx` 和 `CatalogResource` 不拥有 owner。复制或 drop 句柄
  不会关闭 store；创建它们的 owner close 才是清理边界。
- `I18nBuilder::new` 必须接收同一 scope 的 `OwnerAccess` 与错误 reporter。
  store 创建时会校验 error handler；`t!` 创建 computed 时还需要 handler
  仍然可用。handler 被提前释放时，创建新的翻译 memo 会返回 reactive
  handler error。
- `CatalogResource` 的 future、suspense 计数和 catalog 写回都绑定 owner。
  locale 改变会启动针对新 locale 的请求；迟到结果由 core resource 的
  request id 丢弃。`refetch` 使用 cache，`reload` 才绕过 cache。
- 运行时使用 `Rc`、`Cell`、`RefCell` 和 owner-bound future，不应把这些带
  scope 的值当成 `Send + Sync` 的跨线程状态。不同 runtime 的 source、
  suspense 或 binding 会在创建目标节点前被拒绝。
- 翻译结果是普通字符串。插值不会执行 HTML escaping；应把结果作为文本或
  经过应用自己的安全渲染边界处理，不能把不可信翻译直接交给 `innerHTML`。

## 最小可运行流程

下面的源文件同时用于页面展示和 crate 的文档示例测试；它构造内存 catalog，
演示响应式参数与复数翻译，不依赖浏览器。

{% set source = load_data(path="examples/silex_i18n/basic.rs", format="plain") %}
{{ ("```rust\n" ~ source ~ "\n```") | markdown | safe }}

## 专题

- [Locale、catalog 与复数](catalogs.md)：规范化、fallback、placeholder、JSON 和复数规则。
- [Store、翻译 memo 与异步 catalog](runtime.md)：builder、`t!`、策略、cache 和 owner 清理。
- [浏览器、持久化与 Intl](browser.md)：浏览器偏好、`lang/dir`、locale binding 和平台格式化。
- [测试与调试](testing.md)：native、browser、trybuild 和文档示例的验证边界。

## 源码、示例与测试索引

- facade 与 `t!`：`crates/silex_i18n/src/lib.rs`
- catalog、消息 tokenization 与 JSON：`crates/silex_i18n/src/catalog.rs`
- locale 校验与 fallback：`crates/silex_i18n/src/locale.rs`
- 复数类别：`crates/silex_i18n/src/plural.rs`
- builder、store、translation memo 与 resource：`crates/silex_i18n/src/runtime.rs`
- catalog resource 包装：`crates/silex_i18n/src/loader.rs`
- browser metadata 与 locale detection：`crates/silex_i18n/src/browser.rs`
- Intl facade 与 native fallback：`crates/silex_i18n/src/intl.rs`
- 文档示例：`docs/examples/silex_i18n/basic.rs`
- 文档示例测试：`crates/silex_i18n/tests/docs_examples.rs`
- runtime、catalog 和策略测试：`crates/silex_i18n/src/lib.rs` 的 `#[cfg(test)]` 模块
- typed key 测试：`crates/silex_i18n/tests/typed_keys.rs`
- browser/resource 测试：`crates/silex_i18n/tests/wasm.rs`、`tests/browser.rs`
- scope 编译期契约：`crates/silex_i18n/tests/compile_fail.rs`、`tests/ui/`

## 已知限制与维护注意

- locale 解析器只接受 ASCII 字母/数字 subtag，并进行简单的 language、script、
  region 大小写规范化；它不是完整的 BCP 47/Unicode locale negotiation 实现。
- fallback 只按 `Locale::fallback_chain` 和配置的 fallback locale 查找，不会
  自动从所有 catalog 推导语言偏好。浏览器 helper 另外提供 exact、language、
  requested fallback chain 的选择顺序。
- 内置 `plural_category` 只对源码中列出的语言提供专门规则，未列出的语言使用
  one/other 规则；没有基准数据时不要把它描述成完整 CLDR 实现。
- `I18nStore::insert_catalog` 用 catalog 的 locale 替换同 locale 的旧值；内容
  相等时不增加 revision。catalog 被移除后，已有 memo 会重新求值并按缺失 key
  策略返回 key 或空字符串。
- `intl` 的 native 日期格式化固定输出 UTC 文本；wasm 则调用浏览器
  `Intl.DateTimeFormat`，两种平台的显示结果不能视为字节级一致。
- `persist` 的双向同步只负责把 locale signal 与 binding 保持一致，不提供多
  binding 的原子事务；后端本身的安全和冲突语义仍由 `silex_persist` 决定。

修改 locale、catalog、resource 或 browser cleanup 时，应同时核对
`silex_core` 的 owner/resource 契约和本 crate 的 native/wasm/UI 测试；不要只
验证最终字符串而跳过 scope、迟到 completion 和清理路径。
