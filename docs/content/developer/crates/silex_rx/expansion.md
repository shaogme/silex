+++
title = "过程宏展开流程与维护边界"
description = "silex_rx 的 token 解析、响应式源注册、表达式重写和构造路径。"
weight = 10
+++

# 过程宏展开流程与维护边界

`rx!` 的实现集中在 `crates/silex_rx/src/lib.rs`。它不是通用 Rust AST 转换器，而是围绕 `silex_core` owner API 的小型 token/AST 重写器。阅读或修改实现时，应把“宏输入的语法契约”和“生成代码交给 core 处理的运行时契约”分开验证。

## 输入分段

过程宏完整形式有两个分号：

```rust
silex_rx::rx!(silex_core; @ctx ctx; body)
```

`expand` 依次解析：

1. 第一个分号前的 `prefix`，用于拼接生成代码中的 core 路径；
2. `@ctx <ctx>` 段，获取 context 表达式；旧的显式 owner/error handler 形式会被拒绝；
3. 剩余 body，先做 `$` 源标记预处理，再尝试解析为 `syn::Expr`，失败时回退为 `syn::Block`。

缺少任一分段、prefix 为空、context 为空或 body 中使用孤立 `$` 时，过程宏返回带源码 span 的 `syn::Error`，最终由 `proc_macro` 转成编译错误。

## 源注册与去重

`SourceRegistry` 保存三类信息：原始 source token、生成的 marker/reference 标识符，以及在 target owner 中 materialize 后的 promoted 名称。它用 `(SourceKind, source.to_string())` 去重：

- `$count` 先变成类似 `__silex_rx_sig_count` 的 marker，原始 source 是 `count`；
- `$(settings.theme)` 变成编号 marker，原始 source 是 `settings.theme` 的 token；
- 同一 source 在 body 中重复出现时共享一次 `promote` 和一次借用包装；
- shorthand 与 explicit 即使 token 文本相同，也因 `SourceKind` 不同而分别注册。

显式 source 在注册前经过 `validate_source_expression`：允许 `Expr::Path`、`Expr::Field` 和 `Expr::Paren`，递归检查 field 的 base；其它表达式直接报错。这个限制确保 source 可以稳定地作为 `ReactiveSource` 传给 `OwnerAccess::promote`，也避免把有副作用的函数调用重复执行。

## token 与 AST 重写

预处理会递归遍历所有 `Group`，因此 `$` 标记可以出现在 block、tuple、闭包和嵌套宏参数中。之后 `SignalVisitor`：

- 在 `Expr::Path` 的最后一个 segment 上寻找 marker，并改写为借用变量；
- 递归访问普通表达式的子节点；
- 对 `Macro` 的 token stream 使用 `rewrite_tokens`，继续替换嵌套宏中的 marker。

marker 只在本次展开的 `SourceRegistry` 中有效，用户源码中不应手写 `__silex_rx_*` 名称来依赖生成细节。维护 visitor 时要保留宏参数中的替换行为，例如 `format!("{}", $(settings.theme))` 必须和普通表达式一样建立 source 使用记录。

## 生成的四条构造路径

### 常量路径

当 body 没有 active source、不是 `@fn`，并且第一个 token 是可解析的 literal 时，生成：

```text
(|| -> prefix::SilexResult<_> {
    let __silex_owner = ...;
    Ok(__silex_owner.constant(expression)?)
})()
```

空 body 也生成 `constant(())`。这条路径没有依赖边；值仍然由 owner 管理，因此其 `Rx` 仍带 scope lifetime。

### 普通表达式路径

其它非闭包表达式生成 `computed_always`。先对 active source 调用 `promote`，再用递归的 `with` 嵌套读取 promoted source，把读取结果绑定到内部 reference：

```text
(|| -> prefix::SilexResult<_> {
    let promoted = promote(source, error_handler)?;
    Ok(computed_always(move || {
        promoted.with(|__ref_source| {
            Ok(expression_using(__ref_source))
        })?
    }, error_handler)?.into_rx())
})()
```

实际输出会根据 source 数量嵌套多个 `with`，并由 `?` 传播读取错误。`computed_always` 的选择意味着生成值不使用 `PartialEq` 相等门控；通知策略由 `silex_core` 的 computed 实现决定。

### 闭包路径

body 若解析为 closure，宏会强制加入 `move`：

- 无参数 closure 走 `computed_always`，闭包 body 仍会经过 source read 包装；
- 有参数 closure 走 `OwnerAccess::callback`，返回 scope-owned `Callback`，调用时才执行 body。

参数化 closure 不应被文档或调用方当作自动追踪的 computed；它的执行时机由 callback 调用者决定，`Callback::invoke`/`call` 返回的 `SilexResult` 也必须处理。

### `@fn` 路径

body 开头的 `@fn` 在预处理后的 token stream 中被识别并移除，设置 `force_computed`。它让普通表达式跳过 literal constant shortcut，直接走 `computed_always`；对本来就走 computed 的非字面量和无参数 closure，不改变其主要构造器。

## 错误与生命周期

生成代码显式取得：

```text
let __silex_owner = prefix::SilexContextProvider::owner(&(ctx));
let __silex_error_handler: prefix::ErrorReporter<'_> =
    prefix::SilexContextProvider::error_reporter(&(ctx));
```

后续 `promote`、`constant`、`computed_always` 和 `callback` 都使用 `?`，但它们位于宏生成的立即执行闭包中，闭包最终返回 `prefix::SilexResult<_>`。过程宏因此把错误交给宏表达式的调用方，而不是调用方所在函数的隐式提前返回。过程宏本身不创建临时 runtime；编译器和 core runtime 继续负责 owner lifetime、runtime provenance、stale handle、borrow conflict 与 handler 失效。

实现没有 `unsafe`、裸指针或类型擦除。修改生成 closure 的 capture、source_setup 与 nested_reads 的关系时，仍必须复核生成 closure 是否保持 `'owner` 约束，并通过实际编译测试验证 macro hygiene 和错误路径。

## 维护检查点

- 修改输入语法时，同时更新 `preprocess_tokens`、`validate_source_expression`、错误信息和 UI/文档示例。
- 修改 marker 命名或 registry 去重时，检查重复 shorthand、重复 explicit source 和 shorthand/explicit 混用。
- 修改 visitor 时，至少验证字段访问、block、closure 以及 nested macro token stream。
- 修改 constructor selection 时，分别验证 literal、普通 expression、`@fn`、无参数 closure 和参数化 closure。
- 修改生成的 core prefix 或 context 获取方式时，检查 `silex_core::rx!` facade 与直接 `silex_rx::rx!` 调用的路径解析。
