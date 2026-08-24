+++
title = "组件与 PropsBuilder"
description = "silex_macros 的 component 属性宏、PropsBuilder 字段属性和类型状态 builder。"
weight = 10
+++

# 组件与 `PropsBuilder`

`#[component]` 把一个带 `#[ctx]` 参数的 Rust 函数转换为 Props 结构体和
owner-bound View product。底层实现由 `component.rs` 解析函数签名，再委托
`PropsBuilder` derive 生成 builder、显式属性 sink 和 `View` 实现。这样组件的输入
校验发生在编译期，实际挂载和清理由 `silex_dom` 的 owner 负责。

## `#[component]` 输入契约

组件函数必须满足以下条件：

- 必须恰好声明一个 `#[ctx]` 参数；该参数的类型要实现
  `SilexContextProvider<'scope>`，并且不能同时使用 `#[chain]`。
- 参数模式必须是简单标识符。函数不能有 `self` receiver。
- `owner` 和 `error_handler` 是生成函数中的保留局部别名，不能作为普通 prop
  参数，也不能作为 `#[ctx]` 参数名。
- 旧的 `#[inject(...)]` 不再支持；需要 owner 或错误 reporter 时从 `#[ctx]`
  类型提供的 context 取得。
- 宏属性本身不接受参数。链式默认值和输入转换必须写在字段参数上。

组件函数的可见性、泛型和 where-clause 会传给生成的 Props、builder、product
和隐藏 render 函数。返回类型可以是普通 View；如果函数返回 `Result`，或宏能
从最终表达式识别出 `Ok`/`Err` 分支，生成的 render 路径会保留可失败结果。
如果函数声明普通 View 但代码块的最终结果是可识别的 `Result`，宏会把隐藏
render 函数的输出提升为 `SilexResult<View>`。

## 生成的类型

假设函数名为 `Panel`，宏会生成以下主要项目：

| 名称 | 作用 |
| --- | --- |
| `PanelProps` | 保存 context 和所有 prop 的结构体；字段按函数参数顺序生成。 |
| `PanelBuilder<...>` | 保存未完成的 props 和类型状态；声明 `#[attrs]` 时内部收集属性操作。 |
| `PanelComponent` | 持有最终 Props 的 View product；生成后才实现 `View`。 |
| `__silex_render_Panel` | 隐藏 render 函数；解构 Props，恢复 owner/error handler，再执行原函数体。 |
| `Panel(...)` | 构造初始 builder；非 `#[chain]` 参数在这里传入。 |

`PanelBuilder` 的类型参数包含每个 required chain prop 的 `PropMissing` 或
`PropFixed` 状态。每个 required setter 只把对应状态变为 `PropFixed`，因此
字段可以任意顺序设置，但 `.build()` 只为全部 fixed 的 builder 生成。builder
上的 setter 会返回新的 builder；product 不再提供 prop setter，避免已挂载
View 的 props 被静默替换。

## 字段属性

| 属性 | 行为 |
| --- | --- |
| 无属性 | 字段是构造函数参数，直接存入 Props；不会生成链式 setter。 |
| `#[ctx]` | 标记唯一的 context 字段；构造函数接收它，builder 不为它生成 setter。 |
| `#[chain]` | 字段延迟到 builder 设置；没有默认值时是 required prop。 |
| `#[chain(name = method)]` | 延迟字段仍由 builder 设置，但 setter 使用显式的 `method` 名称；也接受字符串形式 `name = "method"`。 |
| `#[chain(each)]` | 仅适用于 `Vec<T>`；setter 改为接收单个 `T`，每次调用追加一个元素。可与 `name`、`default` 组合。 |
| `#[chain(default)]` | 使用字段类型的 `Default`；响应式 wrapper 则从 context owner 创建默认句柄。 |
| `#[chain(default = expr)]` | 缺少字段时执行指定表达式；响应式 wrapper 会把表达式转成 owner-scoped 输入。 |
| `#[attrs]` | 声明唯一的 `AttributeGroup<'scope>` 属性 sink；仅该组件 builder 获得 `.attrs()`、`.class()`、`.attr()` 和事件属性入口。组件函数体必须把它应用到明确元素。 |
| `#[prop(into)]` | 构造函数/setter 接受 `Into<字段类型>`，再保存转换后的值。 |
| `#[prop(render)]` | 将字段作为渲染输入处理；与 `AnyView` 或 View 转换结合时，传入值会被擦除为 scoped `AnyView`。 |
| `#[prop(render_fn(A, B))]` | setter 接受 `Fn(A, B) -> V` 闭包，并用字段类型的 `from_fn` 生成渲染器；至少需要一个参数类型。 |

`#[prop(render_fn(...))]` 不能和 `#[prop(into)]` 或 `#[prop(render)]` 组合。
`#[prop(default)]` 已删除，默认值必须改写为 `#[chain(default)]` 或
`#[chain(default = ...)]`。

链式方法默认使用字段名。需要把存储字段名和调用 API 解耦时，可以显式命名：

```rust
#[component]
fn Menu<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(name = add_item, each)] items: Vec<String>,
) -> impl View<'owner> {
    let _ = (ctx, items);
    children
}

let view = Menu(ctx, AnyView::Empty)
    .add_item("first")
    .add_item("second")
    .build();
```

`name` 只影响生成的 setter 名称，不改变 Props 字段或 render 函数中的绑定名。
同一个 Props 中的链式方法名必须唯一，也不能覆盖生成的 `new` 或 `build` 方法。
组件宏会把参数上的 `#[chain(...)]` 原样传递给生成的 Props，因此该规则同样适用于
`styled!` 复用的组件 Props。

链式字段类型为 `Vec<T>` 时默认仍接收完整的 `Vec<T>`，并保持一次调用替换该字段的
语义。只有显式添加 `#[chain(each)]` 后，setter 才改为接收单个 `T` 并在每次调用时
追加一个元素；`T` 的 `Into` 和 `AnyView` 转换规则仍然适用。例如
`#[chain(name = add_item, each)] items: Vec<String>` 生成的调用形状是
`.add_item("first").add_item("second")`。没有默认值的收集式 Vec 第一次调用仍会把
`PropMissing` 变成 `PropFixed`，之后可以继续调用同一个 setter；有默认值的收集式 Vec
可以完全省略调用。未添加 `each` 时，传入完整 `Vec<T>` 的调用方式保持不变。

## owner-scoped reactive prop

当字段类型是带相同 owner lifetime 的 `Signal`、`ReadSignal`、`Computed`、
`StoredValue` 或 `Rx` 时，setter 可以接受该值类型本身，也可以
接受能实现 `ReactiveInput<'scope, T>` 的普通值。宏通过 `#[ctx]` 取得
`SilexContextProvider::owner` 来完成转换，因此以下两类调用都属于同一个
scope：

```rust
// 这是 API 契约片段；`ctx`、`owner` 和 `builder` 需要由外层测试 scope 提供。
let builder = builder
    .value(10_i32)?
    .value(existing_signal)?;
```

这段 setter 的返回值是 `SilexResult<Self>`，因为创建 reactive input 可能失败；
调用方应传播错误，不能用 `unwrap` 掩盖 owner/runtime 错误。响应式默认值同样
可能让 `.build()` 返回 `SilexResult<PanelComponent>`。`Callback`、`NodeRef`
等 owner-bound 默认值的创建也遵守同一条边界。

## `PropsBuilder` 的直接使用

`#[component]` 内部会生成带 `silex_component` metadata 的 Props derive。需要
手动控制名称时，可以直接使用 `#[derive(PropsBuilder)]`：

```rust
#[derive(Clone, PropsBuilder)]
struct PanelProps<'owner> {
    #[ctx]
    ctx: SilexContext<'owner>,
    children: AnyView<'owner>,
}

fn __silex_render_Panel<'owner>(props: PanelProps<'owner>) -> impl View<'owner> {
    props.children
}
```

上面是 API 契约片段，不是独立 CI 示例。没有 metadata 时，命名按 Props 名称
推导：`PanelProps` 对应 `PanelPropsBuilder`、`PanelComponent`、
`__silex_render_Panel` 和 `Panel(...)`。需要使用不同名称时，metadata 至少
要提供 `builder`、`product` 和 `render`，还可以提供 `constructor` 与 `tag`。
直接 derive 仍必须提供唯一 `#[ctx]` 字段，因为生成的 product 需要从
`MountContext` 取得 owner、逻辑祖先、事务和 error reporter。若需要属性入口，
在结构体字段上声明 `#[attrs] attrs: AttributeGroup<'owner>`；没有该字段时，
builder 不提供通用属性方法。

## View、属性与清理

`PanelComponent` 实现 `View<'scope>`。挂载时会：

1. clone product，并用当前 mount 的 error handler 更新 context 字段；
2. 调用隐藏 render 函数产生已经完成属性选择的真实 View；
3. 将真实 View 以 `context.mount(&view)` 挂载。

只有声明 `#[attrs]` 的 builder 实现属性 builder；普通组件没有隐式属性入口。
组件调用方可在 `.build()` 前使用 `.attrs(group)`, `.class()`, `.attr()` 和
事件方法，组件函数体负责把收到的 `attrs` 应用到指定元素，例如
`div(children).class("panel").apply(attrs)`。实际 attribute operation 的注册、
更新和 cleanup 不由过程宏执行，而由 `silex_dom` 的 owner 和 `AttrOp` 负责。
组件内部创建的 signal、callback、effect 或 DOM 资源必须使用同一个 scope；不要
把借用当前 context 的闭包存入 `'static` 全局状态。

## 对应测试与失败契约

- `pass_component_build_product.rs`：required/default prop 设置和 product 构造。
- `pass_component_required_order.rs`：required setter 可乱序，重复设置最后一次生效。
- `pass_component_build_attributes.rs`：`#[attrs]` builder 的单项、批量和事件属性入口。
- `fail_component_attrs_no_builder.rs`：普通 component 不暴露通用属性 builder。
- `fail_component_attrs_duplicate.rs`：同一 Props 不允许多个 `#[attrs]` 字段。
- `fail_component_attrs_chain_conflict.rs`：`#[attrs]` 不能与 `#[chain(each)]` 组合。
- `pass_component_chain_naming_and_vec.rs`：显式链式方法名、普通 Vec setter 和
  `#[chain(each)]` 的单元素收集。
- `component_chain.rs`：native owner scope 下验证两种 Vec setter 的实际结果。
- `pass_component_fallible_builder.rs`：响应式默认值和 `?` 错误传播。
- `pass_component_standalone_fallback.rs`、`pass_component_explicit_metadata.rs`：
  直接 derive 的命名 fallback 和显式 metadata。
- `fail_component_missing_required_build.rs`：未完成 required prop 不能 build。
- `fail_component_product_required_setter.rs`：product 不暴露 required setter。
- `fail_component_removed_injection.rs`：旧 injection 语法被拒绝。
- `fail_component_raw_props_as_view.rs`：原始 Props 结构体不是 View product。
- `fail_component_fallible_builder_without_question.rs`：可失败 builder 必须处理 `Result`。

完整的 pass/fail 入口和诊断更新流程见[测试与诊断](testing.md)。
