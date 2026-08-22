+++
title = "Store 宏"
description = "#[store] 生成的 owner-scoped reactive field store、快照和句柄组合。"
weight = 20
+++

# Store 宏

`#[store]` 把一个普通的 model 结构体转换为一组 owner-bound reactive field
句柄。它保留原 model 作为快照类型，并生成一个同名 `Store` type alias；每个
字段都是可读写、可建立依赖的 `StoreField`。宏只负责组装和类型约束，signal
创建、读取、写入和 owner 清理由 `silex_core` 完成。

## 输入契约

`#[store]` 不接受宏参数，只支持命名字段结构体：

```rust
#[derive(Clone)]
#[store]
struct Settings {
    theme: String,
    notifications: bool,
}
```

这是与 `crates/tests/silex_macros_test/tests/store_rx.rs` 相同的 API 形状片段，
不是独立的 CI 示例。tuple struct、unit struct、enum、model lifetime `'owner`
以及 model/字段上的 `#[persist(...)]` 都会在编译期被拒绝。

原结构体会原样保留。字段的可见性也保留到生成 Store field 类型中，因此应用
是否能直接访问 `settings.theme` 取决于原字段的 visibility。

## 生成内容

对 `Settings`，宏生成以下稳定使用入口：

| 入口 | 签名/行为 |
| --- | --- |
| `SettingsStore<'owner>` | `SettingsStoreFields` 的 type alias；默认每个字段使用 `Signal<'owner, 字段类型>`。 |
| `SettingsStore::new(owner, source)` | 为 source 的每个字段创建 scoped `Signal`，返回 `SilexResult<Self>`。 |
| `SettingsStore::from_handles(owner, ...)` | 接收可转换为默认 `Signal` 的 `StoreField` 句柄。 |
| `SettingsStore::from_typed_handles(owner, ...)` | 显式指定每个字段句柄类型后组装 Store。 |
| `settings.owner()` | 返回创建该 Store 时使用的 `OwnerAccess<'owner>`。 |
| `settings.snapshot()` | tracked 读取全部字段并重建 `Settings`，返回 `SilexResult<Settings>`。 |
| `settings.snapshot_untracked()` | untracked 读取全部字段并重建 `Settings`，返回 `SilexResult<Settings>`。 |
| `settings.theme` | 访问对应字段句柄；该句柄可使用 `RxRead`、`RxWrite` 和 `ReactiveSource`。 |

生成的内部 `SettingsStoreFields` 是承载实际字段句柄的结构体，并带有
`PhantomData` 保留 model 类型信息。普通代码应优先使用 `SettingsStore` alias；
只有需要自定义句柄类型时才显式使用 `SettingsStore<'owner, ...>` 或调用
`from_typed_handles`。

## 两种构造路径

### 从 model 创建

`new` 对 source 的每个字段调用当前 owner 的 `signal`，因此每个字段拥有
独立的 reactive source：

```rust
let settings = SettingsStore::new(owner, Settings {
    theme: String::from("Light"),
    notifications: false,
})?;

settings.theme.set(String::from("Dark"))?;
let current = settings.snapshot()?;
```

该片段需要外层函数返回 `SilexResult<()>`，并假定 `owner` 已由 runtime scope
提供；它不是独立的 CI 示例。`snapshot()` 对每个字段使用 tracked `get`，
所以如果在 effect 中读取 Store 快照，全部字段都会成为该 effect 的依赖。

### 从已有句柄创建

`from_handles` 适合使用默认 alias：每个输入必须实现
`StoreField<'owner, T>`，并能转换为 `Signal<'owner, T>`。因此普通
`Signal`、以及实现相同契约的其它 scoped 字段句柄都可以参与组装：

```rust
let theme = owner.signal(String::from("Light"))?;
let notifications = owner.signal(false)?;
let settings = SettingsStore::from_handles(owner, theme, notifications)?;
```

需要保留输入句柄的具体类型时，使用 `from_typed_handles`：

```rust
let settings = SettingsStore::<
    '_,
    Persistent<'_, String>,
    Signal<'_, bool>,
>::from_typed_handles(owner, persistent_theme, notifications)?;
```

两段代码都是契约片段；`from_handles` 的输入还必须满足对应的
`Into<Signal>` 约束，而 `from_typed_handles` 只按显式句柄类型检查
`silex_core::StoreField`。Store 宏本身不创建持久化后端，也不解析
`#[persist(...)]`。

## 响应式读取与 snapshot 选择

字段句柄本身可以直接作为 `ReactiveSource` 交给 `rx!` 或其它 core API。只需
观察一个字段时，直接读取该字段可以避免把整个 model 快照注册为依赖；需要
一致地复制整个 model 时使用 `snapshot()`：

```text
settings.theme.get()             -> 只读取 theme 字段
settings.snapshot()              -> tracked 读取全部字段
settings.snapshot_untracked()   -> 读取全部字段但不建立依赖
```

`snapshot_untracked()` 仍然可能因 owner 已关闭或字段读取失败返回
`SilexError`；“untracked”只改变依赖收集，不改变错误处理或生命周期规则。

## lifetime、泛型与清理

- Store 的 `'owner` 是生成字段句柄的作用域，不是 Store 自己拥有的 runtime。
  drop Store 不会关闭 owner；owner 关闭后字段操作按 `silex_core` 句柄契约失败。
- 生成代码为 model 的 lifetime 和 type 参数加入与 `'owner` 兼容的约束，防止
  model 中的借用或类型参数比字段句柄活得更久。
- model 不能声明名为 `'owner` 的 lifetime，因为该名字由宏保留。请改用
  `'model` 等名称；宏会在生成 alias/impl 时建立 `'model: 'owner` 约束。
- Store field 类型实现 `Copy`/`Clone`，复制的是 scoped 能力句柄，不会复制
  runtime owner 或脱离作用域延长资源寿命。

## 对应测试

- `crates/silex_macros/src/store.rs` 的单元测试验证 generic expansion 可解析，
  以及 `#[persist(...)]` 被拒绝。
- `crates/tests/silex_macros_test/tests/store_rx.rs` 验证只读取某一字段时，另一个
  字段的更新不会触发该响应式 effect。
- `crates/tests/silex_macros_test/tests/ui/pass_macro_store.rs` 验证普通 model、
  generic model、const generic、公开字段、已有句柄和 typed handles。
- `crates/tests/silex_macros_test/tests/macro_ui.rs` 负责注册 `pass_macro_*.rs`
  和 `fail_macro_*.rs` 的 trybuild 契约。
