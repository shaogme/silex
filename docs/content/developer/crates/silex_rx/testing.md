+++
title = "测试与调试"
description = "silex_rx 的 token 单元测试、编译期契约和文档示例验证。"
weight = 20
+++

# 测试与调试

`silex_rx` 的正确性不能只靠过程宏 crate 自己的 token 字符串断言。宏还必须在真实的 `silex_core` context、owner lifetime 和错误类型下生成可编译代码，因此测试分成 token 级、trybuild 编译级和文档示例级三层。

## 测试分层

| 位置 | 覆盖内容 | 维护重点 |
| --- | --- | --- |
| `crates/silex_rx/src/lib.rs` 的 `#[cfg(test)]` | `$` 预处理、显式 source 校验、nested macro 重写、重复 source、构造器选择和 `?` 传播。 | 稳定的 token/展开结构；不要把内部 marker 名称当作用户 API。 |
| `crates/tests/silex_macros_test/tests/macro_ui.rs` | `trybuild` 的 pass/compile-fail 宏契约；包括 store 字段、`@fn` 和宏组合。 | 真实 crate 路径、类型推导、lifetime 与诊断信息。 |
| `docs/examples/silex_rx/basic.rs` + `tests/docs_examples.rs` | 直接过程宏、core facade、source promotion 和运行时读取。 | 页面代码必须与编译测试共享同一源码。 |

`silex_rx` 的 source unit tests 位于 proc-macro crate 内，不能替代调用方 crate 的编译测试：过程宏只有在展开点才会遇到真实的 `SilexContextProvider`、trait bound 和 lifetime。

## 文档示例约定

页面通过 Zola `load_data(..., format="plain")` 读取 `docs/examples/silex_rx/basic.rs`。新增或修改示例时，测试入口保持为：

```rust
#[path = "../../../../docs/examples/silex_rx/basic.rs"]
mod basic;

#[test]
fn rx_documentation_example_compiles() {
    assert!(basic::run().is_ok());
}
```

示例文件的公共入口是 `run() -> Result<(), Box<dyn Error>>`，内部 API 错误使用 `?` 传播。只展示不完整上下文的短片段应留在 Markdown fenced code 中，并明确它不是可执行示例；不要把带 `...` 或伪变量的片段放进 `docs/examples/`。

## 推荐验证命令

只验证 `silex_rx` 文档示例时，在仓库根目录运行：

```text
cargo test -p silex_macros_test --test docs_examples
```

该命令会编译示例、展开 `silex_rx` 并执行 `run`，不会运行 workspace 或其它 crate 的测试套件。修改过程宏本身时，可额外运行：

```text
cargo check -p silex_rx
cargo test -p silex_rx
```

仓库级文档变更仍可单独检查格式和站点链接：

```text
cargo fmt --all -- --check
cd docs && zola check
```

## 调试顺序

1. 先确认调用形式：应用 facade 是 `rx!(ctx; body)`，直接过程宏必须是 `rx!(prefix; @ctx ctx; body)`。
2. 如果错误指向 `$`，检查 shorthand 是否为单一标识符，显式 source 是否只是 path/field access。
3. 如果生成代码出现 trait 或 lifetime 错误，确认 context 实现了 `SilexContextProvider`，source 属于同一 runtime；如果调用点使用 `?`，再确认所在函数能传播 `SilexResult`。
4. 如果值不更新，区分 body 是否生成 computed、是否误用了参数化 callback，以及 source 是否在 `$(...)` 中选择了正确的字段值。
5. 如果 nested macro 场景失败，先检查 `rewrite_tokens` 是否记录了 marker 使用，再检查 source_setup 是否只创建了一次 promotion。

## 已知诊断边界

过程宏只负责把 `syn::Error` 转成编译错误；promotion、tracked read、callback invoke 和 owner close 的运行时错误由 `silex_core` 返回或 reporter 接收。调试时不要只比较错误字符串，应根据 `SilexErrorKind` 和 core 的生命周期/错误文档判断是语法、类型、runtime provenance 还是 stale owner 问题。
