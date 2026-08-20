+++
title = "测试与调试"
description = "silex_css 的 native、browser、fallback、trybuild 和文档示例验证方法。"
weight = 60
+++

# 测试与调试

`silex_css` 的正确性同时取决于 Rust 类型约束、CSS 文本净化、响应式
owner、class identity、CSSOM registry 和浏览器 `<style>` 兜底。只验证某条
CSS 字符串相等，不能覆盖 owner dispose、动态 rule 替换或样式表退休。

## 测试分层

| 位置 | 覆盖内容 | 环境 |
| --- | --- | --- |
| `src/**` 单元测试 | escape、layer、class、builder render、类型值、theme diff、template | native |
| `runtime/registry.rs` 测试 | 静态 chunk、顶层规则切分、延迟队列和样式表状态机 | native fake backend |
| `tests/css_type_safety.rs` + `tests/ui/` | 属性值能力、量纲、owner scope、旧 API 和错误诊断 | native trybuild |
| `tests/owner.rs` | inline value、theme、dynamic class、global style、owner cleanup | wasm browser |
| `tests/fallback.rs` | 强制 `<style>` 后端、复用、更新和 dispose | wasm browser + `test-style-fallback` |
| `tests/docs_examples.rs` | `docs/examples/silex_css/basic.rs` 编译并执行 | native |
| `crates/tests/silex_macros_test/` | `css!`、`styled!`、`global!`、`theme!`、`tw!` 展开和 UI 契约 | native trybuild/unit |

native fake backend 只验证 registry 状态转换，不验证浏览器解析、布局或
`adoptedStyleSheets` 的真实实现；修改 `sheet.rs`、DOM 清理或 fallback 时，
必须补 browser 覆盖。

## 常用验证命令

修改文档、Rust 文档示例或 CSS crate 的 native API 时，在仓库根目录运行：

```text
cargo fmt --all -- --check
RUSTFLAGS='-D warnings' cargo check -p silex_css
RUSTFLAGS='-D warnings' cargo test -p silex_css --test docs_examples
zola --root docs check
```

本 crate 的全部 native 单元和 UI 测试可单独运行：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_css
```

只检查浏览器目标的编译、不启动浏览器：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_css --tests \
    --target wasm32-unknown-unknown --no-run
```

浏览器 runner 可用时，运行 owner 测试：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_css --test owner \
    --target wasm32-unknown-unknown
```

强制 fallback 路径：

```text
RUSTFLAGS='-D warnings' cargo test -p silex_css --test fallback \
    --features test-style-fallback --target wasm32-unknown-unknown
```

文档示例只修改 `docs/examples/silex_css/` 时，不需要为了验证它运行
workspace 或其他 crate 的测试；至少运行上面的 `docs_examples` 编译/执行
和 `zola check`。

## 编译期契约

`tests/ui/` 固定的是 CSS crate 最重要的“不应该编译”边界：

- color 不能进入 length、dimension、align-items 等不兼容属性；
- `fr` 不能脱离 grid track，time/length/angle 不能互相混用；
- `calc`、`min`、`max`、`clamp` 不能混合不兼容量纲；
- 动态 CSS child、style、theme 不能逃出 owner scope；
- foreign/invalid signal 和未授权的旧 constructor 不能绕过 source 契约；
- `CssUnsafe`/`raw` 是显式逃生舱，不应被普通 typed API 意外放宽。

修改 `ValidFor`、属性注册表、`IntoCssSource` 或动态签名时，应先增加最小
UI case，再更新实现和精确的 `.stderr`。不要用宽松模式或删除 case 隐藏
类型回归。

## 运行时调试顺序

遇到样式不生效时，按以下顺序缩小范围：

1. 确认是静态 class、动态声明值、动态 selector、theme variable 还是全局
   style；这些路径分别由 registry、inline style、dynamic manager 和 theme
   effect 管理。
2. 检查 owner/runtime 是否仍然 active；动态 CSS 不会创建隐式 runtime，
   foreign runtime 读取会返回错误。
3. 检查元素是否仍保留基础 class、动态 `-d...` class 和预期 inline
   custom property；dispose 后它们消失是预期行为。
4. 检查浏览器 `adoptedStyleSheets`，再检查 `<head>` 下的 `<style>`；后者
   表示走了 fallback，不能用 adopted sheet 查询逻辑判断它不存在。
5. 检查 layer order 和规则内容，不要用 stylesheet 注入先后推断优先级。
6. 如果是动态 selector，检查 `CssPart::SelectorVal` 的 selector 净化；
   如果是声明值，检查 `declaration_value` 和 placeholder 是否发生了二次替换。
7. 如果只在重入/微任务时失败，查看 registry 的 deferred operation 和
   下一轮 flush；借用冲突应排队，不应直接丢样式。

## 文档示例契约

可执行示例只保存在 `docs/examples/silex_css/basic.rs`，页面通过
`load_data(..., format="plain")` 读取同一文件，测试入口是：

```rust
#[path = "../../../docs/examples/silex_css/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented CSS example should compile and run");
}
```

示例的 native 分支只验证 builder 和 owner source；wasm 分支才触及
`web_sys::Element`。页面中的短片段若省略 context、错误处理或 mount owner，
必须明确它不是独立 CI 示例；不要把含 `...`、伪函数或未声明变量的片段放进
`docs/examples/`。

## 失败路径维护清单

- 静态样式注入失败时不能把 ID 标成已注入；重试必须仍然拥有完整 chunk。
- 动态 manager 更新失败时不能替换当前有效 class，也不能遗留半初始化表。
- 共享动态样式表只有最后一个 lease 释放时才能 detach。
- theme 变量名称变化、`None` 和 owner cleanup 都要移除旧变量。
- fallback `<style>` 节点退休后应从 DOM 移除，复用时可以重新挂回同一个
  backend 节点；不会永久积累孤立节点。
- CSS 值构造器的引号、括号、NaN/无穷和错误颜色输入都要有边界测试。
