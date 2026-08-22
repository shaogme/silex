+++
title = "测试与诊断"
description = "silex_macros 的单元测试、trybuild、owner 集成测试和依赖路径验证方法。"
weight = 30
+++

# 测试与诊断

过程宏的主要结果是“调用方能否编译”以及“生成代码是否遵守 owner/runtime
契约”。因此 `silex_macros` 不能只依赖宏 crate 自身的 token 解析测试；组件、
Store、重命名依赖和运行时行为需要在真实 facade 依赖下分层验证。

## 测试分层

| 位置 | 覆盖内容 | 运行环境 |
| --- | --- | --- |
| `crates/silex_macros/src/store.rs` 的 `#[cfg(test)]` | Store expansion 可解析、禁止 persistence 属性。 | native |
| `crates/silex_macros/src/css/**` 的单元测试 | CSS/Tailwind parser、compiler、类型校验和 codegen 中间结果。 | native |
| `crates/tests/silex_macros_test/tests/macro_ui.rs` | `pass_macro_*`、`pass_component_*` 与 `fail_*` 的 trybuild 编译期契约。 | native |
| `crates/tests/silex_macros_test/tests/store_rx.rs` | Store 字段依赖选择、写入、snapshot 和 owner scope。 | native |
| `crates/tests/silex_macros_test/tests/macro_owner.rs` | CSS/global/styled 产物的 owner、动态值、清理和错误边界。 | wasm browser |
| `crates/tests/silex_macros_test/tests/scoped_css_macros.rs` | scoped CSS、Tailwind、classes 和跨 owner 输入。 | native |
| `crates/tests/renamed_dep/` | facade 或 package 重命名后，生成路径仍可解析。 | native |

`macro_ui.rs` 使用 `trybuild::TestCases` 注册通配符，因此新增宏诊断 fixture
必须符合文件名模式，并同步维护 `.stderr`。`.stderr` 不是随意复制的编译器
输出；应先确认诊断对应的是预期 API 契约变化。

## 常用验证命令

仅检查过程宏 crate：

```text
RUSTFLAGS='-D warnings' cargo check -p silex_macros
```

运行宏 crate 自身单元测试：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_macros
```

运行组件、Store 和通用宏的 trybuild：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_macros_test --test macro_ui
```

运行 Store 的 native 集成测试：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_macros_test --test store_rx
```

`macro_owner.rs` 和其中部分宏测试依赖 wasm/browser；应按仓库的 wasm 测试配置
单独编译或运行，不要把 native 通过误认为 DOM 生命周期已经验证。文档站点
检查在仓库根目录执行：

```text
zola --root docs check
```

本次文档没有新增 `docs/examples/` 源码，因此没有额外的 example 编译步骤。
若以后增加文档示例，必须把它接入相应 crate 的测试入口，并只运行该示例
对应的编译/测试，不需要运行 workspace 或其它 crate 的测试。

## 修改宏时的契约清单

- 修改 `#[component]` 签名规则时，覆盖缺少/重复 `#[ctx]`、receiver、保留参数名、
  旧 injection、generic 和 fallible builder。
- 修改 Props 字段属性时，覆盖 required setter 顺序、重复 setter、默认表达式、
  显式链式方法名、`#[chain(each)]` 的 Vec 单元素收集、普通 Vec 完整值 setter、
  `Into`、`render_fn` 参数约束、reactive input 和 owner scope escape。
- 修改 `PropsBuilder` codegen 时，确认 builder 的 `PropMissing`/`PropFixed` 状态、
  product 的 `View` 实现、pending attributes 和 error reporter 传递仍然一致。
- 修改 `#[store]` 时，覆盖 named fields、model generic/lifetime、默认 RwSignal、
  `from_handles`、`from_typed_handles`、tracked/untracked snapshot 和 persist 拒绝。
- 修改 `crate_path` 时，覆盖默认 facade、`package = "silex"` 重命名、宏在 facade
  自身内部使用，以及 Store 的 `silex_core` 路径。
- 修改 CSS 或 route 入口时，分别回到其独立专题文档和对应测试矩阵；本 crate 总览
  不重复维护 CSS/route 语法说明。

## 编译失败的诊断顺序

1. 先确认调用方启用了对应 feature，并检查宏入口是否因此被 cfg 排除。
2. 对 `#[component]`，检查是否只有一个 `#[ctx]`，以及 context 类型是否实现
   `SilexContextProvider<'scope>`。
3. 对 builder，查看错误类型中缺少的 `PropFixed` 状态；这通常表示某个 required
   `#[chain]` 字段没有设置，或可失败 setter 的 `Result` 没有传播。
4. 对 reactive input/default，确认输入和目标字段共享同一 owner lifetime，并从
   context owner 创建，而不是把另一个 runtime 的句柄传入。
5. 对 Store，确认字段句柄实现 `StoreField<'owner, T>`，model lifetime 没有使用
   保留名 `'owner`，并区分 `snapshot()` 的 tracked 读取和 untracked 读取。
6. 对生成路径错误，检查 Cargo manifest 中实际依赖名称；重命名 facade 时使用
   对应的 renamed-dependency fixture 复现。
7. 对运行时 DOM/CSS 问题，检查生成代码传入的 owner、error handler 和 pending
   attributes，再查看 `silex_dom`/`silex_css` 的结构化错误或 cleanup report。

## 已知测试边界

- trybuild 固定的是编译期 API 和诊断；它不能证明 browser DOM、stylesheet 注入或
  owner cleanup 的运行时顺序。
- `pass_component_chain_naming_and_vec.rs` 固定链式 API 的三个边界：显式命名必须
  生成对应 setter，普通 Vec 接受完整值，`#[chain(each)]` Vec 接受单个元素并允许
  重复调用。
- `component_chain.rs` 在 native owner scope 中读取生成 product 的 Props，验证普通
  Vec 的整体替换和 `#[chain(each)]` Vec 的调用顺序。
- native Store 测试能验证响应式依赖选择，但不能替代跨 runtime 句柄拒绝和 DOM
  资源清理测试。
- `crate_path` 的 `OnceLock` 只缓存当前 rustc 进程内的路径解析结果；这属于宏
  编译期优化，不应被解释为运行时全局注册表或应用级缓存。
- 目前没有经验证的宏编译耗时、生成代码体积或运行时吞吐基准，文档不对这些指标
  作数值承诺。
