+++
title = "Store、翻译 memo 与异步 catalog"
description = "silex_i18n 的 builder、响应式翻译、缺失策略、catalog cache 和 owner-scoped loader。"
weight = 20
+++

# Store、翻译 memo 与异步 catalog

`I18nStore` 是一个带 scope 的 `Copy` 能力句柄。它把 locale、fallback locale、
catalog registry 和一个 revision signal 组合起来；它不拥有这些 signal 的
生命周期。应用应在拥有目标组件/页面的 `OwnerAccess` 中创建 store，并将同一
scope 的错误 reporter 传给 builder。

## Builder 与初始值

```rust
let store = I18nBuilder::new(owner, handler.view())
    .locale(Locale::new("zh-CN")?)
    .fallback_locale(Locale::new("en-US")?)
    .catalog(chinese_catalog)
    .catalog(english_catalog)
    .missing_key(MissingKeyPolicy::ReturnKey)
    .missing_argument(MissingArgumentPolicy::KeepPlaceholder)
    .build()?;
```

builder 的 locale 选择顺序是：

1. `persist` feature 下的 `locale_binding` 当前值；
2. `.locale(...)` 显式值；
3. 第一个传入 catalog 的 locale；
4. `Locale::new("en")`。

没有显式 fallback 时，fallback locale 等于最终选出的 locale。多个 catalog 使用
同一个 locale 时，builder 依次写入 registry，后一个值替换前一个值。

`I18nBuilder::new` 没有 `Default`，也不能脱离 `OwnerAccess` 构造。这个 API 约束
确保 store 中的 signal、computed、错误 handler 和后续 cleanup 属于同一 owner。
若传入的 `Persistent`、source 或其它 scope 值来自不同 runtime，builder 会在
创建目标节点前返回 `RuntimeMismatch`，不会留下半初始化的节点。

## 同步翻译与 `t!`

直接调用 `translate_now` 适合事件处理器或只需要一次快照的代码：

```rust
let text = store.translate_now(
    "welcome.user",
    &[Argument::new("name", "Alice")],
)?;
```

响应式 UI 通常使用 `t!`。它在 owner 中创建 always-notifying computed，并在
computed 闭包内读取参数表达式：

```rust
let name = owner.signal(String::from("Alice"))?;
let greeting = t!(store, "welcome.user", name = name.get())?;

assert_eq!(greeting.get()?, "Hello, Alice!");
name.set(String::from("Bob"))?;
assert_eq!(greeting.get()?, "Hello, Bob!");
```

`t!` 有两种形式：

- `t!(store, "key", name = value)` 构造 `Argument` 列表；每个 value 在
  computed 内求值，因此响应式读取会成为依赖。
- `t!(store, variant)` 接受实现 `I18nVariant` 的值，通常由 `I18nKeys` 派生；
  variant 的 `arguments` 和可选 `count_name` 参与翻译。

两种形式都会跟踪当前 locale、fallback locale 和 catalog revision。locale
改变、fallback 改变或不同内容的 catalog 被插入/移除后，已有翻译 memo 会重新
计算。相同 catalog 再次插入不会增加 revision，也不会无谓地重新运行 memo。

缺 key 和缺 argument 是独立策略：

| 配置 | 默认行为 | `Empty` 行为 |
| --- | --- | --- |
| `MissingKeyPolicy` | 返回 key 本身 | 返回空字符串 |
| `MissingArgumentPolicy` | 保留 `{name}` | 删除 placeholder |

这些缺失情况不是 `Result` 错误；locale/catalog/runtime 错误仍然通过
`SilexResult` 返回，并由 computed 的 error handler 接收延迟错误。

## Catalog registry 与动态替换

`has_catalog` 只查询指定 locale 是否存在。`insert_catalog` 按 catalog.locale
替换已有值；只有新值与旧值不相等时才递增 revision。`remove_catalog` 删除
locale 后递增 revision，已有翻译会根据 fallback chain 再查找，最终按
`MissingKeyPolicy` 处理。

这套 revision 是响应式失效通知，不是 catalog 版本协商或跨线程锁。调用方在
多个 owner 之间共享 catalog 时，应在每个 store 中显式插入，并自行决定加载和
替换顺序。

## `CatalogResource`

`I18nStore::catalog_resource` 将 loader 封装成当前 locale 驱动的 core
`Resource`：

```rust
let resource = store.catalog_resource(
    |locale| async move { load_catalog(locale).await },
    CatalogResourceOptions::new(),
)?;
```

loader 必须是 `'static` 的 `Fn(Locale) -> Future<Output = Result<Catalog, E>>`，
`E` 需要 `Clone + Debug + 'static`。resource 首次运行时先查询 store cache；
命中 cache 时不调用 loader。loader 返回的 catalog.locale 必须与请求 locale
完全相等，否则状态为 `CatalogLoadError::LocaleMismatch`。

| 方法 | 语义 |
| --- | --- |
| `state()` | 读取 `ResourceState<Catalog, CatalogLoadError<E>>`。 |
| `value()` / `get_data()` | 读取当前成功 catalog 的 clone；无数据时为 `None`。 |
| `loading()` | 读取当前是否正在加载。 |
| `refetch()` | 重新触发 resource，但允许使用已有 catalog cache。 |
| `reload()` | 标记本次请求绕过 cache，再触发 resource。 |
| `resource()` | 取得底层 `silex_core::Resource`。 |

请求成功后，store 内部 effect 将 catalog 写回 registry，从而使 `t!` 创建的
memo 也能看到新内容。请求替换、owner close 和可选 `SuspenseContext` 的计数/清理
由 `silex_core::Resource` 负责；`CatalogResource` 不提供独立的 cancel API。

`CatalogResourceOptions::suspense` 必须来自同一 runtime。scope 不匹配会在分配
resource 节点前返回 fatal error；不要把其它页面的 suspense context 混进当前 store。

## 性能边界

`translate_now` 每次调用都会读取当前 locale、fallback locale、catalog revision，
再按候选 chain 查找消息；渲染每个 placeholder 时还会在传入的 `Argument` 列表中
查找同名参数。源码没有为这些路径提供基准数据或复杂度保证。渲染循环中应复用
`t!` 创建的 `Rx<String>`，不要在每次视图更新时重复构造临时 store、catalog 或
翻译输入；大量 catalog 的异步 cache 也会复制成功 catalog 的值后写回 registry。

## 持久化 locale

启用 `persist` 后，可以把 `Persistent<Locale>` 交给
`I18nBuilder::locale_binding`。构造时 binding 值优先于 builder locale；随后
store locale 与 binding 通过两个 owner-bound effect 双向同步，并对相等值抑制
重复写入。

binding 只改变 locale 的持久化/外部同步来源，不替代 `I18nStore` 的 locale signal，
也不提供多 key 原子事务。backend 的读写错误仍属于 `silex_persist` 的边界，
最终会转换为 i18n error。binding 必须与 store 在同一 runtime 和兼容 scope 内。

## 清理与错误诊断

owner close 会清理 locale/fallback signal、revision、translation computed、
resource future、binding effect 和 catalog loader 的 completion。句柄被 drop
不会触发这些清理；不要把 `I18nStore` 当作 RAII owner。

排查翻译问题时按以下顺序检查：

1. `Locale::as_str()` 与 `fallback_chain()` 是否符合预期；
2. `has_catalog` 确认命中的 locale 是否已加载；
3. `Catalog::get` 确认 key 和 placeholder 是否正确；
4. `MissingKeyPolicy`/`MissingArgumentPolicy` 是否掩盖了资源问题；
5. `SilexError` 中的 reactive/runtime error 是否来自 owner close 或 runtime mismatch；
6. 异步场景确认是 cache 命中、`refetch` 还是 `reload`，并检查 resource state。

## 对应源码与测试

- `crates/silex_i18n/src/runtime.rs`：builder、registry、memo、resource 和 binding。
- `crates/silex_i18n/src/loader.rs`：`CatalogResource` 与 loader error。
- `crates/silex_i18n/src/lib.rs`：同步翻译、revision、policy 和 scope 测试。
- `crates/silex_i18n/src/lib.rs` 与 `tests/typed_keys.rs`：typed variant 的 memo 行为。
- `crates/silex_i18n/tests/wasm.rs`：异步 loader、cache、reload 和 suspense 测试。
