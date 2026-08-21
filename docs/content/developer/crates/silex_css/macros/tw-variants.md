+++
title = "tw_variants! 与 declare_variants!"
description = "tw_variants! 的 variant schema、生成类型、字符串解析、compound 和 CSS 合并契约。"
weight = 80
+++

# `tw_variants!` 与 `declare_variants!`

`tw_variants!` 把一组 Tailwind class 按 variant 槽位组织成可复用 schema。
过程宏实现位于 `crates/silex_macros/src/css/tw/variants.rs`，最后展开为
`silex_css::declare_variants!` 和每个 option 的 `tw!` 调用。
`declare_variants!` 本身是 `silex_css/src/tw/variants.rs` 中的
`macro_rules!`，负责生成 enum、Config、`IntoClass` 和 `VariantSchema`。

## 推荐的 item 形式

```rust
tw_variants! {
    pub struct ButtonStyle {
        base: "inline-flex items-center rounded-md",
        variants: {
            size: {
                sm: "h-8 px-3 text-sm",
                "icon-lg": "size-10 p-0",
            },
            tone: {
                neutral: "bg-muted text-foreground",
                accent: "bg-primary text-white",
            }
        },
        default_variants: { size: "sm", tone: "neutral" },
        compound_variants: [
            { tone: "accent", size: "icon-lg", class: "shadow-lg" }
        ]
    }
}
```

这是依赖 `tw` feature 的语法示意。item 形式把类型定义放到调用方作用域，
所以生成的 `ButtonStyle`、`ButtonStyleSize` 等类型可以出现在字段、函数
签名和组件 props 中。变体名必须能转换成 Rust 字段名；option 名可以是
字符串，kebab-case、空格和数字会转换为 PascalCase Rust 标识符，数字开头
会加 `Val` 前缀。

每个 option 的 class string 会单独调用 `tw!` 编译。`base` 总是先写入，
然后按声明顺序写 variant option，最后写命中的 compound class；这个顺序
由 `declare_variants!` 的 `write_class` 固定，不依赖样式表首次注入顺序。

## 表达式形式

不写 `pub struct Name` 时，宏接受相同 DSL 并在一个表达式 block 中生成
helper，返回 `Helper::new()`：

```rust
let styles = tw_variants! {
    base: "inline-flex",
    variants: { size: { sm: "p-2", lg: "p-4" } },
    default_variants: { size: sm }
};
let class_name = styles.get("lg");
```

该形式的生成类型位于 block 内，不能命名，也不能放进外部结构体字段或
签名；需要跨函数传递时使用 item 形式。

## 生成 API

每个 variant 会生成一个 enum 和一个 schema 字段。item 形式的命名规则是
`<StructName><VariantNamePascalCase>`，例如 `ButtonStyleSize`。schema 实例
提供：

- `new()`：用 `default_variants` 或该槽位的第一个 option 初始化；
- `with_<variant>(EnumValue)`：编译期类型安全的链式设置；
- `class()`：渲染当前 enum 配置的完整 class 字符串；
- `get(option_1, ...)`：从运行时字符串渲染，未知值回退到默认值；
- `get_checked(option_1, ...)`：严格字符串解析，未知值返回
  `UnknownVariantOption`；
- `get_opt(option_1, ...)`：`None` 使用默认值，其它值走宽松字符串转换。

option enum 还提供 `OPTIONS`、`OPTION_KEYS`、`try_from_str` 和
`FromStr`。空字符串代表“未指定”，会选择默认值；非空未知值只有严格
API 才会返回错误，`From<S>`/`get` 则按设计回退默认值。

字符串比较会忽略大小写、空白以及 `-`/`_` 分隔符。因此 DSL 中的
`"icon-lg"` 可以由运行时的 `icon_lg` 选中；但两个 option 不能在 PascalCase
或规范化字符串 key 上发生冲突，宏会在编译期拒绝。

## compound 与预编译合并

`compound_variants` 的条件必须引用已声明的 variant 和合法 option，字段名
可以使用 identifier 或字符串。条件全部命中时追加其 class。宏还会分析
不同槽位写入的 CSS 属性集合：互相覆盖的槽位/compound 会被折成组合表，
最多预编译 256 个组合；超出上限会要求把冲突属性移入 `base` 或拆开槽位。
不相互覆盖的槽位保持独立，减少组合数量。

不要把这个合并机制与 `tw!` 条件 tuple 混为一谈：前者为 variant schema
预生成配置组合，后者为 owner 运行时条件切换 class。

## 与 `declare_variants!` 的直接边界

直接使用 `declare_variants!` 时，调用方必须提供已经生成好的 enum 类型和
每个 option 的 class expression，例如：

```rust
declare_variants! {
    pub struct ManualStyle {
        base: "base",
        variants: {
            pub tone: ManualTone [default = Neutral] = {
                Neutral => "tone-neutral",
                Accent => "tone-accent",
            }
        }
    }
}
```

通常不应手写这层；`tw_variants!` 会负责 option key、`tw!` class 编译、
PascalCase 冲突和 compound 校验。直接宏适合已经有稳定 class expression
或需要测试 runtime schema 的底层场景。

## 维护与测试

修改变体宏时应覆盖：item/表达式两种形式、数字和 kebab option 名、未知
default/compound 引用、宽松/严格字符串 API、class 写入顺序以及组合上限。
过程宏测试在 `crates/silex_macros/src/css/tw/variants.rs`；runtime 的
`declare_variants!` 顺序和解析测试在 `crates/silex_css/src/tw/variants.rs`。
`crates/tests/renamed_dep/` 还验证生成代码不依赖调用方把 facade 恰好命名
为 `silex`。
