+++
title = "classes!"
description = "classes! 如何把普通和条件 class 表达式展开为可清理的 AttributeGroup。"
weight = 40
+++

# `classes!`

`classes!` 是 class attribute 的轻量组合宏，入口在
`crates/silex_macros/src/css/classes.rs`。它不编译 CSS，也不生成 stylesheet；
职责只是把每一项转换成 `silex_view::attributes::AttrOp`，并装进
`AttributeGroup`。

## 输入与展开

```rust
let attrs = classes![
    "button",
    extra_classes,
    "is-active" => is_active,
];
```

片段依赖 `IntoStorable`、`ApplyToDom` 和当前 owner，不能作为独立 example。
宏把每一项解析成两种 AST：

- `expr`：普通 class 表达式；
- `class_expr => condition_expr`：条件 class。

每项都会生成等价于以下流程的代码：

```text
class expression / (class, condition)
            │ IntoStorable
            ▼
ApplyToDom::into_op(..., ApplyTarget::Class)
            │
            ▼
AttributeGroup(Vec<AttrOp>)
```

最终 class 值的类型能力来自 `silex_dom` 的 `IntoStorable`/class adapter，
而不是 `classes!` 自己复制一套字符串转换规则。条件表达式由相应的
reactive storable 绑定处理；owner cleanup 会移除该项写入的 class。

空输入 `classes![]` 直接展开为 `AttributeGroup::default()`。

## 与 `css!`/`tw!` 的边界

`classes!` 只组合已经存在的 class 文本：

- 它不会为任意字符串生成 CSS；
- 它不会解析 Tailwind utility；
- 它不会计算 CSS layer 或注入 stylesheet；
- 它只把 class operation 放入 attribute group。

需要从 utility 生成 CSS 时使用 `tw!`；需要声明 CSS block 时使用 `css!`。
如果一个 class 本身来自 `tw!`、`tw_variants!` 或静态样式宏，可以把返回值
作为 `classes!` 的普通表达式输入。

## 维护与测试

修改时要保持每一项都经由 `IntoStorable` 和
`ApplyToDom::into_op(..., ApplyTarget::Class)`，不要在过程宏中直接拼接
字符串，否则会绕开响应式 source 和 cleanup。当前单元测试
`classes_converts_inputs_through_into_storable` 检查这条展开契约；集成行为
在 `crates/tests/silex_macros_test/tests/scoped_css_macros.rs` 的
`classes_converts_signal_to_a_scoped_attribute_group` 覆盖。
