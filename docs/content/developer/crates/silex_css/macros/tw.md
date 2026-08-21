+++
title = "tw!"
description = "tw! 的 utility 解析、variant、条件 class、动态 arbitrary value 和 owner 更新路径。"
weight = 70
+++

# `tw!`

`tw!` 是 Tailwind utility 的过程宏入口，只有 `silex_macros/tw` feature
开启时可用。它在编译期把 utility 字符串解析为 `UtilityRule`，解析 modifier
和 arbitrary value，再借用 CSS compiler 生成静态 class、CSS layer 和必要
的动态绑定。实现位于 `crates/silex_macros/src/css/tw.rs` 及 `css/tw/`。

## 静态输入

```rust
let class_name = tw!(
    "inline-flex items-center gap-2 rounded-md",
    "hover:bg-blue-600 md:px-6"
);
```

每个字符串 literal 都是一段以空白分隔的 utility list；逗号只分隔
`tw!` 的输入段。无条件输入会走普通 `generate_css_output`：静态规则
进入 `utilities` layer，返回包含生成 class 的字符串。`group`、`peer`、
`container` 等 marker utility 会保留为原始 class，并与生成 class 一起返回。

modifier 解析顺序是：生成表中的内置 modifier、配置中的自定义 breakpoint、
函数式 modifier，最后对未知前缀报错并给出建议。未知 variant 不会静默
变成任意 pseudo-class；需要自定义 selector 时应使用 Tailwind 支持的
`[&:my-pseudo]:...` 形式。

当前函数式 modifier 包括 `supports-[...]`、`min-*`、`max-*`、`nth-*`、
`in-*`、`not-*` 和 `starting`。`min-[600px]`/`max-[600px]` 使用排序可控
的 width condition；配置的 `[theme.breakpoints]` 也可作为 breakpoint 名。

## 动态 arbitrary value

Tailwind 字符串中的 `$(expr)` 或 `$path` 可以进入 arbitrary value，例如：

```rust
let dynamic = tw!(error_handler; "w-[$(width)]");
```

这里的 `error_handler;` 是显式错误边界，语义与 `css!` 相同。动态 arbitrary
value 会走 `DynamicCss` 的属性值 getter 和 CSS custom property；它必须属于
当前 owner，并且不能让 source 逃出 owner lifetime。动态 selector 仍遵守
CSS compiler 的 selector 净化边界。

## 条件 tuple

条件输入支持两种参数顺序，else 分支可省略：

```rust
let classes = tw!(
    "transition-colors",
    (is_primary, "bg-blue-600 text-white", "bg-slate-200 text-black"),
    ("opacity-50", is_disabled),
);
```

解析器也接受 `("then", condition, "else")`。条件为字面量 `true`/`false`
时在宏阶段折叠，不生成 owner effect；其它条件会转成当前 owner 上的
`IntoCssReactive<bool>`，运行时通过 `AttrOp::on_commit` 安装初始 CSS：

1. 读取所有条件并选择完整 class 字符串；
2. 只添加新 token、移除旧 token；
3. 记录当前 class，owner cleanup 时移除剩余 token。

条件分支中的 then/else utility 必须是静态 CSS；不能把动态 arbitrary value
或静态插值放进条件分支。需要动态值时，把它移到条件外的普通 `css!`/`tw!`
路径。

## 冲突属性的编译期合并

条件段可能同时写入相同 CSS 属性。宏先用 `WriteSet` 找到会互相覆盖的
segment 簇，再预编译每个条件组合的完整 utility class，把覆盖顺序固定在
编译期；互不冲突的段保持独立 class。一个簇最多允许 6 个条件分支，超过
上限会报错并要求拆分调用。过渡/动画控制属性会被提升为常驻段，避免 class
切换时把 transition 本身一起移除。

这项合并只解决同一 `tw!` 调用中可分析的冲突；不同调用或手写 class 的
层次仍由 CSS layer 和调用方组合决定，不应依赖首次渲染顺序。

## CSS 生成与 layer

Tailwind resolver 生成的 CSS block 使用 `CssCompiler::compile_block`，其
前缀是 `slx-tw-`，进入 `utilities` layer。variant/selector 会按 modifier
priority 排序，LightningCSS 负责最终解析、压缩和目标浏览器输出。静态
样式用 registry ID 去重；条件路径在 effect 内首次为实际组合注入对应 class
样式。

## 维护与测试

修改 `tw!` 时要同步检查 parser、resolver、modifier priority、WriteSet 合并
和 `generate_css_output`：

- 输入 span 必须尽量指向具体 utility/variant；
- 未知 modifier 必须报错，不能让 LightningCSS 接管一个永远不匹配的伪类；
- dynamic arbitrary value 不能进入条件 branch 的静态缓存；
- class 更新和 cleanup 必须保持 token 级增删，不影响调用方其它 class；
- 修改 layer 或 hash 时要覆盖静态 registry、条件组合和 owner 测试。

`crates/silex/tests/tw_tests.rs` 覆盖 verbose/utility 结果，
`crates/tests/silex_macros_test/tests/scoped_css_macros.rs` 覆盖动态 CSS、
条件 class 和动态 `tw!`，`crates/silex_macros/src/css/tw/tests/` 覆盖 parser、
resolver、modifier 和回归用例。
