+++
title = "组件与 PropsBuilder"
description = "silex_macros 的 component 属性宏、PropsBuilder 字段属性和类型状态 builder。"
weight = 10
+++

# 组件与 `PropsBuilder`

`#[component]` 把一个带 `#[ctx]` 参数的 Rust 函数转换为 Props 结构体和
owner-bound View product。底层实现由 `component.rs` 解析函数签名，再委托
`PropsBuilder` derive 生成 builder、属性转发和 `View` 实现。这样组件的输入
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
| `PanelBuilder<...>` | 保存未完成的 props、pending attributes 和类型状态。 |
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
| `#[chain(default)]` | 使用字段类型的 `Default`；响应式 wrapper 则从 context owner 创建默认句柄。 |
| `#[chain(default = expr)]` | 缺少字段时执行指定表达式；响应式 wrapper 会把表达式转成 owner-scoped 输入。 |
| `#[prop(into)]` | 构造函数/setter 接受 `Into<字段类型>`，再保存转换后的值。 |
| `#[prop(render)]` | 将字段作为渲染输入处理；与 `AnyView` 或 View 转换结合时，传入值会被擦除为 scoped `AnyView`。 |
| `#[prop(render_fn(A, B))]` | setter 接受 `Fn(A, B) -> V` 闭包，并用字段类型的 `from_fn` 生成渲染器；至少需要一个参数类型。 |

`#[prop(render_fn(...))]` 不能和 `#[prop(into)]` 或 `#[prop(render)]` 组合。
`#[prop(default)]` 已删除，默认值必须改写为 `#[chain(default)]` 或
`#[chain(default = ...)]`。

## owner-scoped reactive prop

当字段类型是带相同 owner lifetime 的 `Signal`、`ReadSignal`、`RwSignal`、
`Computed`、`StoredValue` 或 `Rx` 时，setter 可以接受该值类型本身，也可以
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
直接 derive 仍必须提供唯一 `#[ctx]` 字段，因为生成的 product `View::mount`
需要从它取得 owner 和 error reporter。

## View、属性与清理

`PanelComponent` 实现 `View<'scope>`。挂载时会：

1. clone product，合并构造阶段和挂载阶段收到的 pending attributes；
2. 用当前 mount 的 error handler 替换 context 中的 reporter；
3. 调用隐藏 render 函数产生真实 View；
4. 将 pending attributes 传给 `silex_dom::view::View::mount`。

builder 和 product 都实现属性 builder，因此组件可以在 build 前或 build 后接收
class、style、property 和 event 操作。实际 attribute operation 的注册、更新和
cleanup 不由过程宏执行，而由 `silex_dom` 的 owner 和 `AttrOp` 负责。组件内部
创建的 signal、callback、effect 或 DOM 资源必须使用同一个 scope；不要把借用
当前 context 的闭包存入 `'static` 全局状态。

## 对应测试与失败契约

- `pass_component_build_product.rs`：required/default prop 设置和 product 构造。
- `pass_component_required_order.rs`：required setter 可乱序，重复设置最后一次生效。
- `pass_component_build_attributes.rs`：builder/product 的属性转发。
- `pass_component_fallible_builder.rs`：响应式默认值和 `?` 错误传播。
- `pass_component_standalone_fallback.rs`、`pass_component_explicit_metadata.rs`：
  直接 derive 的命名 fallback 和显式 metadata。
- `fail_component_missing_required_build.rs`：未完成 required prop 不能 build。
- `fail_component_product_required_setter.rs`：product 不暴露 required setter。
- `fail_component_removed_injection.rs`：旧 injection 语法被拒绝。
- `fail_component_raw_props_as_view.rs`：原始 Props 结构体不是 View product。
- `fail_component_fallible_builder_without_question.rs`：可失败 builder 必须处理 `Result`。

完整的 pass/fail 入口和诊断更新流程见[测试与诊断](testing.md)。
