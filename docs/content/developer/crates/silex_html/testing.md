+++
title = "测试、验证与已知限制"
description = "silex_html 的文档示例、标签宏编译验证、代码生成复核和风险定位。"
weight = 50
+++

# 测试、验证与已知限制

`silex_html` 自身主要是生成代码和属性 trait facade。验证应分开看：标签
函数/宏是否能编译、属性 facade 是否映射到预期的 attribute name，以及真实
DOM mount/owner cleanup 是否符合 `silex_dom` 契约。没有浏览器时不要把 native
构造测试误认为 DOM 集成测试。

## 测试分层

| 位置 | 覆盖内容 | 环境 |
| --- | --- | --- |
| `docs/examples/silex_html/basic.rs` | HTML 宏、void 函数、属性 trait、SVG 函数的最小组合 | native 可编译/运行构造路径 |
| `crates/silex_html/tests/docs_examples.rs` | 通过 `#[path]` 引入上面的同一份源码 | `cargo test -p silex_html --test docs_examples` |
| `crates/silex_html/src/tags/*.rs` | 当前生成的函数、marker 和宏产物 | `cargo check -p silex_html` |
| `crates/silex_dom/tests/**` | mount、namespace、attribute、事件、owner cleanup 和失败回滚 | native / wasm 按测试文件划分 |
| `crates/utils/silex_codegen/src/tags*.rs` | 标签解析、patch、DOM type mapping 和宏文本 | codegen crate 的检查/生成流程 |

文档页面通过 `load_data(..., format="plain")` 读取 `basic.rs`，所以不要在
Markdown 中再复制一份会独立演进的 Rust 示例。缺少上下文的 API 片段应使用
普通 fenced code，并明确它不是 CI 示例。

## 本 crate 的最小验证

新增或修改 `docs/examples/silex_html/` 后，只需先运行对应的 crate 级示例
测试：

```text
RUSTFLAGS=-D warnings cargo test -p silex_html --test docs_examples
```

只修改 `silex_html` 的标签 facade 或文档时，不需要为了这项工作运行
workspace 或其他 crate 的测试。若修改了生成器输入/输出，还应补充：

```text
RUSTFLAGS=-D warnings cargo check -p silex_html
cargo run -p silex_codegen
```

其中 `cargo run -p silex_codegen` 会写入多个产物，不应作为只读文档变更的
默认步骤；只有标签生成链本身变更时才运行并审阅完整 diff。

站点页面检查使用：

```text
zola --root docs check
```

## 示例测试边界

示例 native 分支只构造 view，因为 `TypedElement` 的构造不会访问浏览器对象；
真正的 `View::mount` 需要 `Document`、host node 和 `MountedApp`，由
`silex_dom` 的 wasm/browser 测试负责。若未来在本 crate 加入 browser 示例，
应单独配置 `wasm_bindgen_test_configure!(run_in_browser)`，并覆盖：

- HTML 元素使用 HTML namespace；SVG 元素使用 SVG namespace；
- void 标签没有 child，non-void 标签的 child 按顺序挂载；
- attribute facade 生成预期 attribute，property 使用者明确调用 `prop`；
- owner dispose 后节点、listener 和响应式属性不再更新；
- mount 部分失败时 provisional owner 和已追加节点都能回滚。

## 当前风险定位

### SVG 宏展开失败

`src/tags/svg.rs` 的生成宏引用不存在的 `$crate::view_chain!`。定义宏本身
不会触发名称解析，只有调用 `svg!(...)`、`g!(...)` 等宏时才会出现编译错误。
在修复 codegen 前，使用 `svg(child)`、`g(child)` 等函数形式；修复后应把
`svg!(path())` 加入文档示例测试，防止回归。

### SVG `<a>` marker 类型不正确

`codegen.rs` 的 `dom_type` match 先处理 `"a"`，导致 SVG `a` 使用
`HtmlAnchorElement`。这会影响依赖 `Tag::DomElement` 的 typed cast 或
`NodeRef`；普通 SVG attribute 仍可通过通用 `attr` 使用。修复前不要把该
marker 当作 SVG 专用类型安全保证。

### 属性 trait 不是内容模型校验

`FormAttributes`、`AnchorAttributes` 等使用 blanket impl，`FormTag` 等
marker 没有参与 trait bound。因此测试应验证“方法映射和运行时 DOM 目标”，
而不是期待 `div().value(...)` 在编译期被拒绝。若未来收紧约束，必须新增
compile-fail 测试并评估自定义 view/attribute 转发的兼容性。

## 复核清单

修改 `silex_html` 时至少确认：

- 新增的标签函数是 void 还是 non-void，签名和文档一致；
- HTML/SVG namespace 没有通过错误的构造器混用；
- Rust keyword、HTML/SVG 同名函数和 crate 根宏导出没有冲突；
- 命名 attribute 使用正确的 HTML name，属性和 property 没有混写；
- 值类型满足 `IntoStorable<'scope>`，借用没有逃逸 scope；
- 文档示例来自唯一源码，并已执行 `docs_examples`；
- 生成产物的改动可以从 `tags.rs`/`codegen.rs` 解释，而不是手工补丁。
