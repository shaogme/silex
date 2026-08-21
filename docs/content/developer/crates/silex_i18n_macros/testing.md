+++
title = "测试与诊断"
description = "silex_i18n_macros 的 trybuild、fixture、生成代码和 catalog schema 验证方法。"
weight = 10
+++

# 测试与诊断

过程宏的主要行为发生在编译期，因此 `I18nKeys` 不能只用一个运行时断言验证。
本 crate 用 trybuild 将可通过输入、编译错误和稳定诊断分开；运行时参数/复数
效果则由 `silex_i18n` 的 typed key 测试覆盖。

## 测试分层

| 位置 | 覆盖内容 |
| --- | --- |
| `tests/derive.rs` | 注册所有 `pass_*.rs` 和 `fail_*.rs` UI fixture。 |
| `tests/ui/pass_*.rs` | 正常 enum、重命名 facade、generic、raw identifier、字段名碰撞和 runtime translation。 |
| `tests/ui/fail_*.rs` | path、runtime path、重复属性、missing key、placeholder、tuple、count 和 plural schema 错误。 |
| `tests/ui/*.stderr` | 预期编译诊断；诊断文字变化需要人工复核。 |
| `tests/fixtures/*.json` | canonical catalog 与 plural placeholder mismatch 输入。 |
| `crates/silex_i18n/tests/typed_keys.rs` | 生成的 `I18nVariant` 如何参与响应式 memo、fallback 和 catalog revision。 |

## 常用验证命令

在仓库根目录运行：

```text
RUSTFLAGS=-D warnings cargo fmt --all -- --check
RUSTFLAGS=-D warnings cargo check -p silex_i18n_macros
RUSTFLAGS=-D warnings cargo test -p silex_i18n_macros
RUSTFLAGS=-D warnings cargo check -p silex_i18n --features macros
RUSTFLAGS=-D warnings cargo test -p silex_i18n --features macros --test typed_keys
zola --root docs check
```

trybuild 失败时，先确认错误属于预期契约，再决定是否更新对应 `.stderr`。不要
用宽松匹配或删除 fixture 来掩盖宏生成代码的变化。若只修改文档而不修改 fixture，
不需要运行 workspace 或其它 crate 的测试。

## 修改宏时的契约清单

- 修改属性语法时，同时覆盖重复属性、未知属性和缺失属性；
- 修改 catalog walker 时，覆盖 nested key、message/object collision、空 key、
  invalid placeholder、未知 plural category 和缺少 `other`；
- 修改 codegen 时，覆盖 unit/named variant、generic、raw identifier、字段名与
  生成局部变量冲突，以及 `crate` facade path；
- 修改 plural 校验时，覆盖 count 默认值、自定义 count 名称、数值 primitive、
  非数值字段和不同 form 的 placeholder 集合；
- 修改 runtime path 解析时，覆盖包重命名、`silex` facade 和显式 crate path；
- 修改 `Argument` 生成时，确认 field value 被 owned `String` 捕获，不让借用逃出
  variant 或 computed 调用边界。

## 调试顺序

1. 先检查 path 是相对于宏调用 package 的 manifest，还是因为 manifest path 不存在而回退到源文件目录。
2. 将 canonical JSON 展开为实际 leaf key，确认 enum `key` 没有把 object 中间节点当消息。
3. 比较 placeholder 的集合而非出现顺序；每个复数 form 必须使用相同集合。
4. 复数 variant 先确认 catalog 被识别为 plural，再检查 `count` field 名称和类型。
5. runtime path 报错时，检查依赖是否名为 `silex_i18n`/`silex`，否则显式设置 `crate`。
6. 生成 impl 编译失败时，再查看 `I18nVariant` facade 是否同时暴露 `Argument` 和 trait。

## 已知限制

- 宏只检查一个 canonical catalog；它不会比较其它 locale 文件的 key、placeholder
  或复数 form。多 locale 一致性需要额外的构建步骤或测试。
- path 是构建输入，但宏不会监听外部翻译平台或执行运行时热更新；运行时 catalog
  仍由 `silex_i18n` loader/cache 管理。
- placeholder 只支持源码规定的 ASCII identifier，宏不解析 ICU、Markdown、HTML
  或富文本语法。
- count 类型检查是语法匹配，不会验证传入数值的量纲、范围或 locale 规则；最终
  category 选择仍由 runtime 的 `plural_category` 完成。
