+++
title = "silex_i18n_macros"
description = "从 canonical JSON catalog 生成并校验类型化翻译 variant 的过程宏。"
template = "section.html"
sort_by = "weight"
+++

# `silex_i18n_macros`

`silex_i18n_macros` 只提供 `I18nKeys` derive macro。它在编译期读取一个 JSON
catalog，把 enum variant 与消息 key、placeholder 和复数 count 对齐，并为 enum
实现 `silex_i18n::I18nVariant`。它不加载运行时 catalog、不创建 owner、不执行
翻译，也不替代 `silex_i18n` runtime。

## 在 Silex 架构中的位置

```text
canonical JSON catalog + enum definition
                  │ derive(I18nKeys)
                  ▼
       compile-time schema validation
                  │
                  ▼
       I18nVariant::key / arguments / count_name
                  │
                  ▼
       silex_i18n::t!(store, variant)
```

## 稳定入口与最小声明

过程宏入口是 `#[derive(I18nKeys)]`，支持两个位置的 `#[i18n(...)]` 属性：

```rust
use silex_i18n::I18nVariant;
use silex_i18n_macros::I18nKeys;

#[derive(I18nKeys)]
#[i18n(path = "locales/en-US.json")]
enum Text {
    #[i18n(key = "home.title")]
    HomeTitle,
    #[i18n(key = "welcome.user")]
    WelcomeUser { name: String },
    #[i18n(key = "cart.items", count = "count")]
    CartItems { count: u32 },
}
```

这段是 API 片段，不是独立的 CI 文档示例：`locales/en-US.json` 必须替换为
调用方 package 中实际存在的 canonical catalog。仓库内可编译的正反例位于
`crates/silex_i18n_macros/tests/ui/`，运行时使用示例位于
`crates/silex_i18n/tests/typed_keys.rs`。

container 属性：

| 属性 | 要求 |
| --- | --- |
| `path = "..."` | 必填；指向编译期 JSON catalog。重复声明会报错。 |
| `crate = "..."` | 可选；指定生成代码使用的 runtime path，例如 `silex_i18n` 或 `crate::facade::i18n`。 |

variant 属性：

| 属性 | 要求 |
| --- | --- |
| `key = "..."` | 每个 variant 必填；必须存在于 catalog leaf 中。 |
| `count = "..."` | 仅复数 key 可用；指定承载复数数字的字段名，默认是 `count`。 |

## Catalog 路径与编译期读取

宏首先用 `CARGO_MANIFEST_DIR` 加上 `path` 查找文件；找不到时再使用属性
span 的源文件目录作为 fallback。文件必须存在、能读取且根节点必须符合 JSON
catalog 结构。生成代码还包含 `include_str!`，因此 catalog 文件会进入生成项
的编译依赖；修改 catalog 会触发使用该 derive 的 crate 重新编译。

路径读取发生在编译器进程中，不是运行时网络访问。请将 catalog 放在源码仓库
中，并把它视为构建输入；不要把用户可修改的运行时文件当作 canonical schema。

JSON 的 schema 规则与 runtime catalog 基本一致：嵌套 object 产生点号 key；
复数 object 必须有 `other`，且每种 form 必须是字符串；消息/object path 冲突、
空 object key、未知 plural category 和非法 placeholder 都会在编译期报错。

## 生成的 `I18nVariant`

宏为原 enum 保留 generics/where clause，并生成：

- `key(&self) -> &'static str`：返回 variant 对应的 canonical key；
- `arguments(&self) -> Vec<Argument>`：named field 按字段声明顺序转换为
  `Argument::new(field_name, field_value)`；字段值立即通过 `ToString` 变成 owned string；
- `count_name(&self) -> Option<&'static str>`：普通消息为 `None`，复数消息返回
  `count` 或 `#[i18n(count = "...")]` 指定的名字。

生成的 impl 只依赖 runtime path，不会把 catalog 内容装载进 `I18nStore`。应用仍
需用 `Catalog::from_entries`、`Catalog::from_json` 或自己的 loader 将运行时
catalog 提供给 store，然后调用：

```rust
let translated = t!(store, Text::WelcomeUser {
    name: String::from("Alice"),
})?;
```

## 编译期拒绝的输入

- derive 目标不是 enum；
- 缺少 container `path`、variant `key`，或 key 为空/不存在；
- tuple variant；variant 字段集合与 catalog placeholder 集合不完全相同；
- 复数 form 的 placeholder 集合不一致；
- 复数消息缺少 count 字段、count 字段不是支持的数值 primitive，或非复数消息声明了 count；
- 重复的 `path`、`crate`、`key` 或 `count` 属性；
- runtime path 无法解析，且自动发现不到 `silex_i18n` 或 `silex` facade；
- catalog 文件不存在、JSON 无效、path collision 或 plural schema 无效。

数值 count 类型是源码显式识别的 `i8` 到 `i128`、`isize`、`u8` 到 `u128`、
`usize`、`f32`、`f64`，以及这些类型的引用/括号包装；任意实现了数值 trait
的自定义类型不会自动通过该检查。

## runtime path 与重命名依赖

未设置 `crate` 时，宏通过 `proc_macro_crate` 先查找 `silex_i18n`，再查找
`silex` facade，并生成对应路径。依赖被 Cargo 重命名、应用使用自定义 facade
或宏位于同一 crate 时，自动发现可能不符合预期；此时显式设置：

```rust
#[i18n(
    path = "locales/en-US.json",
    crate = "my_i18n"
)]
```

`crate` 的值会按 syn `Path` 解析；它必须指向包含 `I18nVariant` 和 `Argument`
的 runtime facade。

## 对应源码与测试

- proc-macro 入口：`crates/silex_i18n_macros/src/lib.rs`
- schema 读取、属性解析和 codegen：`crates/silex_i18n_macros/src/i18n_keys.rs`
- canonical catalog：`crates/silex_i18n_macros/tests/fixtures/en-US.json`
- plural schema 反例：`crates/silex_i18n_macros/tests/fixtures/plural-mismatch.json`
- pass/compile-fail 契约：`crates/silex_i18n_macros/tests/ui/`
- trybuild 入口：`crates/silex_i18n_macros/tests/derive.rs`
- runtime typed key：`crates/silex_i18n/tests/typed_keys.rs`

专题测试说明见[测试与诊断](testing.md)。
