+++
title = "tw_verbose!"
description = "tw_verbose! 的编译期诊断内容、target 输出路径和与 tw! 的语义一致性。"
weight = 90
+++

# `tw_verbose!`

`tw_verbose!` 是 `tw!` 的诊断版本，入口仍在
`crates/silex_macros/src/lib.rs`，实现通过 `css/tw.rs` 调用同一条内部路径，
只是把 `verbose` 标记设为 true。它不改变 class、CSS layer、条件更新或
错误处理语义；排查 utility 解析、modifier 排序和条件组合时可以临时替换
`tw!` 使用。

## 输入与输出

输入语法与 `tw!` 完全相同，包括：

- 一个或多个 utility string literal；
- `(condition, then, else)` 或 `(then, condition, else)` 条件 tuple；
- `error_handler;` 前缀；
- 动态 arbitrary value 的 `$` 插值（条件 branch 仍不能包含动态值）。

编译成功时，宏除了生成正常的 class/CSS 代码，还输出诊断内容：

- `Macro Input`：宏接收到的 token 文本；
- `Generated CssBlock AST`：无条件路径的 CSS block；
- `Compiled Class Name`、`Static CSS`、`Component CSS`：无条件路径产物；
- 条件路径中每个缓存/组合 class 的编译 CSS。

## 诊断文件

诊断正文写入：

```text
<CARGO_TARGET_DIR>/silex-tw-debug/<stable-hash>.txt
```

没有设置 `CARGO_TARGET_DIR` 时，宏从 `CARGO_MANIFEST_DIR` 向上查找最近的
`target/silex-tw-debug/`。文件名由输入内容的稳定 CSS hash 生成，重复构建
会覆盖同一个输入的文件，而不是无限追加新文件。编译器 stderr 只保留一行
文件路径指针，便于并行 Cargo 输出和 JSON message format 消费。

如果 target 目录不可创建或写入，诊断不会让编译失败，而是退回把完整正文
写到 stderr。这条 fallback 只影响可观测性，不改变宏的实际展开结果。

## 与 `tw!` 共享的边界

`tw_verbose!` 和 `tw!` 共用 parser、resolver、modifier schema、CSS compiler、
条件分支合并和 owner-bound `AttrOp`。因此：

- unknown modifier、非法 utility 和错误 arbitrary value 的编译失败应与
  `tw!` 一致；
- verbose 输出不能为了展示 AST 而走另一条 CSS 生成路径；
- 条件组合仍受单簇 6 个条件和 variant 合并上限约束；
- 动态值仍需显式 handler，并受当前 owner lifetime 约束。

## 维护与测试

如果修改 verbose section，先检查无条件和条件两条路径是否都记录信息，且
写文件失败仍回退 stderr。`crates/silex_macros/src/css/tw/verbose.rs`
覆盖稳定 hash 与“不 panic”测试；`crates/silex/tests/tw_tests.rs` 提供实际
宏调用。诊断文件是 target 生成物，不应提交到仓库，也不应被文档测试作为
稳定路径断言。
