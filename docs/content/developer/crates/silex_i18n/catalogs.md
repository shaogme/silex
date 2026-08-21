+++
title = "Locale、catalog 与复数"
description = "silex_i18n 的 locale 规范化、消息 tokenization、JSON catalog 和复数选择。"
weight = 10
+++

# Locale、catalog 与复数

`Locale` 和 `Catalog` 是 `silex_i18n` 的数据边界。locale 在进入 runtime 前
会被校验并规范化；catalog 在构造时就解析 placeholder 和复数结构。这样翻译
阶段不需要再次解析原始模板，格式错误会以 `I18nError` 暴露在加载/构造边界。

## Locale 规范化与 fallback

```rust
let locale = Locale::new("zh_hant_tw")?;
assert_eq!(locale.as_str(), "zh-Hant-TW");
assert_eq!(locale.language(), "zh");
```

`Locale::new`/`Locale::parse` 会拒绝空字符串、空 subtag、空白/控制字符以及
非 ASCII 字母数字 subtag。第一个 subtag 是 1 到 8 个 ASCII 字母；下划线会
转换为连字符。规范化规则是：language 小写、四字母 script 首字母大写、两字母
region 或三位数字 region 大写，其它 subtag 保留原样。

`fallback_chain` 从完整 locale 逐级删去末尾 subtag：

```text
zh-Hant-TW → zh-Hant → zh
```

store 翻译时先遍历当前 locale 的 chain，再遍历 fallback locale 的 chain，并用
集合避免重复候选。找到消息后就停止；因此 fallback 是“按 key 找消息”，不是
把两个 catalog 合并成一个 catalog。

## 构造 catalog

最小的内存 catalog 如下：

```rust
let catalog = Catalog::from_entries(
    Locale::new("en-US")?,
    [
        ("home.title", "Home"),
        ("welcome.user", "Hello, {name}!"),
    ],
)?;
```

`Catalog::from_entries` 要求 key 非空且不能重复。文本模板会被拆成
`Segment::Literal` 与 `Segment::Argument`；placeholder 名称只能由 ASCII 字母、
数字和下划线组成，且不能以数字开头。缺少右花括号、孤立右花括号和空
placeholder 都会返回 `InvalidMessage`。

`Catalog::get` 返回已解析的 `Message`，`len`/`is_empty` 只描述该 locale 的
消息数量。`Catalog` 是值类型，插入 store 后 store registry 会保存它的 clone；
外部继续持有原值不会影响 registry。

## 复数消息

使用 `CatalogValue::plural` 或 `PluralForms::from_templates` 构造复数消息：

```rust
let items = CatalogValue::plural([
    ("one", "You have {count} item."),
    ("other", "You have {count} items."),
]);
let catalog = Catalog::from_entries(Locale::new("en")?, [("cart.items", items)])?;
```

所有复数 forms 都必须有 `other`；category 只能是 `zero`、`one`、`two`、`few`、
`many` 或 `other`。`Message::plural` 会从每个 form 的共同 placeholder 推断
count name：优先使用 `count`，否则只有一个共同 placeholder 时使用它，否则回退
到 `count`。调用 `translate_now` 时，显式传入的 variant count name 可以覆盖
这个推断结果。

`plural_category` 当前按 `Locale::language()` 选择源码中实现的规则。阿拉伯语、
俄语/乌克兰语/白俄罗斯语、波兰语、捷克语/斯洛伐克语、斯洛文尼亚语、罗马尼亚语、
立陶宛语、拉脱维亚语、爱尔兰语、威尔士语、法语/葡萄牙语有专门分支；其它语言
使用整数 1 为 `one`、其余为 `other` 的规则。非有限数和无法解析的 count 使用
`other`。

当消息来自 fallback catalog 时，复数规则使用实际命中的 catalog locale，而
不是当前请求 locale。比如当前 locale 是 `zh-CN`、只命中 `en` catalog 时，
`en` 的 plural rule 决定 form。

## JSON feature

启用 `json` 后，`Catalog::from_json` 接受对象根节点，并把嵌套对象展平为点号
key：

```json
{
  "home": { "title": "Home" },
  "cart": {
    "items": {
      "one": "{count} item",
      "other": "{count} items"
    }
  }
}
```

上例产生 `home.title` 和 `cart.items`。一个对象一旦包含任意已知复数 category，
就会按复数对象处理：所有字段必须是已知 category，且必须包含 `other`，每个值
必须是字符串。普通对象不能与消息 leaf 冲突，例如同时存在 `home: "Home"`
和 `home.title: "Title"` 会返回 `InvalidCatalog`。

JSON parser 只负责把合法 JSON 变成 `Catalog`，不会检查多语言 catalog 之间的
placeholder 是否一致；如果需要跨 locale 的 schema 契约，应使用
[`I18nKeys`](../silex_i18n_macros/) 的 canonical catalog 校验。

## 错误边界与维护

locale、模板和 catalog 结构错误都是可恢复的 `I18nError`。调用方应在应用加载
或构造阶段处理 `Result`，不要用 `unwrap` 隐藏翻译资源损坏。runtime、owner、
不同 runtime 的 source 等错误则会以 fatal 的 reactive `I18nError` 返回；两类
错误的区别见 [`silex_core` 错误文档](@/developer/crates/silex_core/errors.md)。

catalog 内容最终是字符串。tokenizer 不执行 HTML 转义，也不理解 ICU message
语法或富文本标签；如果业务需要富文本，应在更高层定义安全的结构化消息协议，
不要将翻译字符串直接当作 HTML。

## 对应源码与测试

- `crates/silex_i18n/src/locale.rs`：locale 校验、规范化和 fallback chain。
- `crates/silex_i18n/src/catalog.rs`：消息 tokenization、JSON flatten 和 collision 检查。
- `crates/silex_i18n/src/plural.rs`：category enum 与语言规则。
- `crates/silex_i18n/src/lib.rs`：fallback、插值、复数和 JSON 行为测试。
