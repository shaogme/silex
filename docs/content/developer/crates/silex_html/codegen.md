+++
title = "标签代码生成链"
description = "silex_html 从 MDN compatibility data 到 HTML/SVG Rust 源码的生成流程。"
weight = 40
+++

# 标签代码生成链

`silex_html` 的标签集合由 `crates/utils/silex_codegen` 生成。理解生成链很
重要：`src/tags/html.rs` 和 `src/tags/svg.rs` 是产物，手工编辑会在下一次
codegen 时丢失；标签命名、void 判定、能力 trait 和宏冲突处理都来自生成器。

## 输入和中间结构

`tags.rs` 读取 `mdn_compat_data.json` 的 `html.elements` 与 `svg.elements`，
解析为两个 `Vec<TagDef>`：

```text
Mdn compatibility JSON
          │
          ▼
TagConfig { html, svg }
          │  apply_memory_only_patches
          ▼
TagDef { struct_name, tag_name, func_name, is_void, traits }
          │
          ▼
   html.rs / svg.rs
```

`build_tag_list` 先对 JSON key 排序，再用 `heck::AsPascalCase` 生成
`struct_name`。原始 tag name 不做 keyword 清理；这一步留给
`apply_memory_only_patches`，以便同时处理 marker 名和函数名。

## 内存 patch

`apply_memory_only_patches` 不改 JSON 文件，只改本次生成使用的内存配置：

- `Type`、`Box`、`Loop`、`If`、`For`、`Option`、`Data` 等冲突 marker 改为
  `TypeEl`、`BoxEl`、`LoopEl`、`IfEl`、`ForEl`、`OptionTag`、`DataTag` 等；
- 对应的函数名例如 `type_el`、`loop_el`、`option_tag`、`data_tag`；
- SVG 的 `a`、`script`、`style`、`title` 使用 `SvgA`、`SvgScript`、
  `SvgStyle`、`SvgTitle` 等 marker，避免与 HTML 语义混淆；
- HTML 标签按名称补充 `FormTag`、`LabelTag`、`AnchorTag`、`MediaTag`、
  `OpenTag`、`TableCellTag`、`TableHeaderTag`；
- 非 void 标签统一添加 `TextTag`；SVG 标签统一添加 `SvgTag`。

void 判定不是从 JSON 的内容模型推导的：HTML 使用
`HTML_VOID_ELEMENTS`，SVG 使用 `SVG_SHAPE_ELEMENTS`。因此维护者必须以
生成配置和生成函数签名为准，不能仅根据熟悉的 HTML/SVG 规则猜测某个函数
是否接受 child。

## `codegen.rs` 的输出

`generate_module_content` 对每个 `TagDef` 生成：

1. 一个 `silex_dom::define_tag!` 调用；
2. HTML 使用 `new`，SVG 使用 `new_svg`；
3. HTML 普通元素默认 `web_sys::HtmlElement`，特殊控件使用更具体的
   `web_sys` 类型；SVG 默认使用 `web_sys::SvgElement`；
4. non-void 标签的 `macro_rules!`，宏展开为 `$crate::html::...` 或
   `$crate::svg::...` 的函数调用。

HTML 生成后，`main.rs` 会收集 HTML 宏名，并把它传给 SVG 生成器。SVG
生成器会给冲突的宏和函数加 `svg_` 前缀，避免 `macro_export` 把 HTML 和
SVG 的同名宏同时导出到 crate 根。函数在 `html`/`svg` 模块内，不共享同一
命名空间；宏则是 crate 根导出，碰撞规则只针对宏。

## 运行生成器

生成器会同时刷新 HTML/SVG 标签和 CSS/Tailwind 产物，输入数据必须已经存在：

```text
cargo run -p silex_codegen
```

如果需要重新下载 MDN/CSS 数据，使用显式 fetch 模式：

```text
cargo run -p silex_codegen -- --fetch
```

`--fetch` 会写入 `crates/utils/silex_codegen/data/`，属于外部数据变更；
不要在只想修正标签命名时无意刷新整个数据集。生成器从 workspace 根运行，
也支持在 `silex_codegen` crate 目录运行；路径选择逻辑见 `main.rs`。

## 维护顺序

修改标签时按以下顺序检查：

1. 先确认 `mdn_compat_data.json` 中的 HTML/SVG key 和大小写；
2. 再确认 `tags.rs` 的 PascalCase、keyword patch、void 列表和 trait patch；
3. 再确认 `codegen.rs` 的 DOM type mapping、HTML/SVG macro namespace 和
   `$crate` 路径；
4. 运行 codegen，检查 `src/tags/html.rs`、`src/tags/svg.rs` 的 diff；
5. 编译调用函数和宏的文档示例；真实 mount 行为由 `silex_dom` 测试覆盖。

生成产物中的 `#[rustfmt::skip]` 是生成器主动写入的，不能通过手工格式化
产物来“修复”问题。若生成器输出变化是预期行为，应在同一变更中更新文档、
示例和测试。

## 生成器测试契约

`tags/codegen.rs` 的单元测试固定了 HTML/SVG namespace 对应的 DOM 类型，
包括 HTML `a` 的 `HtmlAnchorElement` 和 SVG `a` 的 `SvgaElement`，同时
固定生成宏使用 `$crate::chain!`。修改 namespace、DOM type mapping 或宏模板
时，应先更新这些测试，再检查生成产物和 `silex_html` 文档示例。
