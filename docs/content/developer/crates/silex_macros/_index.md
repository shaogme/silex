+++
title = "silex_macros"
description = "Silex 组件、PropsBuilder、Store 以及 CSS 和 route 宏的编译期生成边界。"
template = "section.html"
sort_by = "weight"
+++

# `silex_macros`

`silex_macros` 是 Silex 的过程宏 crate。它在调用方编译期间解析函数、结构体
或宏输入，校验声明，并生成 `silex_core`、`silex_dom`、`silex_css` 或
`silex_router` 能消费的类型和实现。它不创建 `Runtime`，不持有组件状态，也不
直接执行 DOM、CSS 或路由操作；这些行为发生在生成代码调用对应运行时 API 时。

## 在 Silex 架构中的位置

```text
调用方源码
  #[component] / #[derive(PropsBuilder)] / #[store]
  css! / styled! / router!
          │
          ▼
      silex_macros
  syn 解析 · 编译期校验 · token 生成
          │
          ▼
  silex_core · silex_dom · silex_css · silex_router
  owner 生命周期 · View · 响应式句柄 · 运行时注册
```

过程宏生成的代码属于调用方 crate，因此调用方的 feature、依赖名称、公开性和
生命周期都会影响最终能否编译。宏通过 `proc_macro_crate` 解析 `silex` 或
`silex_core` 的真实依赖路径，不能假定 Cargo 依赖一定使用默认名称。

## 稳定入口

| 入口 | feature | 生成内容 |
| --- | --- | --- |
| `#[component]` | `component` | Props 结构体、带类型状态的 builder、View product 和组件构造函数。 |
| `#[derive(PropsBuilder)]` | `component` | 从命名字段结构体生成 builder、product、View 和属性转发实现。 |
| `#[store]` | `store` | 以 `RwSignal` 或兼容的 `StoreField` 句柄承载字段的 scoped Store。 |
| CSS 宏 | `css` / `tw` | 详见 [CSS 宏文档](@/developer/crates/silex_css/macros/_index.md)。 |
| `router!` | `route` | 详见 [router 宏与类型生成](@/developer/crates/silex_router/macros.md)。 |

`default` feature 同时启用 `component`、`css`、`tw`、`store` 和 `route`。关闭
默认 feature 时，只有对应 feature 的过程宏入口和实现会被编译；`tw` 还依赖
`css`。

## 一次展开的生命周期

```text
宏调用
  │  编译期
  ├─ 解析输入与属性
  ├─ 拒绝不完整或互相矛盾的声明
  └─ 生成调用方可见的类型与实现
          │  运行时
          ▼
调用方创建 Runtime/Owner/Context
          │
          ▼
生成的 View、Store 或 route API 使用 owner-scoped 运行时句柄
```

生成代码中的 `'owner`/`'scope` 不是宏自己创建的生命周期。它们必须来自调用
方的 `OwnerAccess`、`SilexContextProvider` 或其它上层 owner-bound 类型；owner
关闭后，生成的 signal、callback、View 和 Store 句柄遵守对应运行时的失效与
清理规则。过程宏没有跨线程或 `Send + Sync` 的额外保证。

## 最小调用形状

下面是与 `crates/tests/silex_macros_test/tests/ui/pass_component_build_product.rs`
相同 API 形状的契约片段。它用于说明输入和生成结果，不是独立的 CI 示例；可
编译的正反例以该测试目录中的 fixture 为准。

```rust
#[component]
fn SaveButton<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(default)] label: String,
) -> impl View<'owner> {
    let _ = (owner, label);
    children
}

// 在拥有 `SilexContext` 和错误处理器的 owner scope 中：
let view = SaveButton(ctx, AnyView::Empty)
    .label("Save")
    .build();
```

`#[component]` 会把 `ctx` 转为 Props 中的 context 字段，并在隐藏的 render
函数中恢复 `owner` 和 `error_handler` 这两个局部别名。组件调用返回 builder；
只有所有没有默认值的 `#[chain]` 字段都已设置时，`.build()` 才存在。

## 主题文档

- [组件与 PropsBuilder](component.md)：`#[component]`、字段属性、builder 状态、View product 和 owner 边界。
- [Store 宏](store.md)：`#[store]` 生成的字段句柄、快照、泛型和持久化组合方式。
- [测试与诊断](testing.md)：trybuild、运行时集成测试、重命名依赖和修改契约后的验证顺序。
- [CSS 宏](@/developer/crates/silex_css/macros/_index.md)：CSS、styled、theme、Tailwind 等入口。
- [router 宏与类型生成](@/developer/crates/silex_router/macros.md)：route tree、path、matcher 和 table 生成。

## 源码与测试索引

- 过程宏入口：`crates/silex_macros/src/lib.rs`
- 组件生成：`crates/silex_macros/src/component.rs`
- Props builder 生成：`crates/silex_macros/src/props_builder.rs`
- Store 生成：`crates/silex_macros/src/store.rs`
- 依赖路径解析：`crates/silex_macros/src/crate_path.rs`
- 组件和通用宏的 trybuild 入口：`crates/tests/silex_macros_test/tests/macro_ui.rs`
- owner/runtime 集成测试：`crates/tests/silex_macros_test/tests/macro_owner.rs`
- Store 响应式测试：`crates/tests/silex_macros_test/tests/store_rx.rs`
- CSS 宏作用域测试：`crates/tests/silex_macros_test/tests/scoped_css_macros.rs`

## 已知限制与维护注意

- `#[component]` 必须恰好有一个 `#[ctx]` 参数；参数模式必须是简单标识符，
  不接受 receiver、显式 `owner`/`error_handler` 参数或旧的 `#[inject(...)]`。
- `#[store]` 只接受命名字段结构体，不接受 model 或字段上的 `#[persist(...)]`；
  需要持久化时，应先构造 `Persistent` 句柄，再用 `from_handles` 或
  `from_typed_handles` 组装 Store。
- 生成代码会把 owner lifetime 传播到 builder、View product、Store 字段和属性
  操作。不要把这些对象或闭包提升到 owner 之外；编译期 scope error 是保护，
  不是可以通过类型擦除绕过的运行时提示。
- `crate_path` 的自动发现依赖调用方 Cargo manifest。重命名 facade、在 facade
  自身内部调用宏或使用非标准依赖布局时，应检查对应的 renamed-dependency
  fixture；自动发现失败时，生成代码可能退回默认路径并在调用方给出解析错误。
- 本 crate 的实现没有直接使用 `unsafe`；但生成代码会调用带 owner、DOM 和
  错误处理边界的运行时 API，修改展开顺序时必须同时验证 cleanup、错误传播和
  scope 约束。
