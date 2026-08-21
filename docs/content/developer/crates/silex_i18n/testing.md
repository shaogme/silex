+++
title = "测试与调试"
description = "silex_i18n 的 native、browser、JSON、持久化、Intl 和 trybuild 验证方法。"
weight = 40
+++

# 测试与调试

国际化行为横跨纯数据解析、响应式 memo、owner cleanup、异步 catalog、浏览器
metadata 和可选持久化。只断言一次 `translate_now` 的字符串，不能覆盖 locale
切换、迟到 loader、scope escape 或 DOM 属性恢复。

## 测试分层

| 位置 | 覆盖内容 | 环境/feature |
| --- | --- | --- |
| `src/lib.rs` 单元测试 | locale、fallback、插值、复数、policy、revision、runtime mismatch | native；JSON/persist 按 feature |
| `src/catalog.rs`、`src/browser.rs`、`src/loader.rs`、`src/intl.rs` | tokenizer、浏览器选择、loader error、native Intl fallback | native |
| `tests/typed_keys.rs` | `I18nKeys` 生成的 variant、参数 memo、fallback 与 catalog replacement | `macros` |
| `tests/compile_fail.rs` + `tests/ui/` | builder 必须有 owner/handler、旧构造方式拒绝、scope 与 view 边界 | trybuild；browser view case 需 `browser-tests` |
| `tests/wasm.rs` | CatalogResource、cache/reload、suspense、future drop | wasm browser |
| `tests/browser.rs` | storage/query binding、metadata stack、DOM 翻译和浏览器 locale | wasm browser + `browser-tests` |
| `tests/docs_examples.rs` | `docs/examples/silex_i18n/basic.rs` 的可执行文档示例 | native |

`tests/wasm.rs` 与 `tests/browser.rs` 使用 `wasm_bindgen_test_configure!(run_in_browser)`；
native `cargo test` 不会执行这些 browser case。trybuild 的 `.stderr` 是编译期
契约的一部分，只有诊断契约确实改变时才更新。

## 常用验证命令

在仓库根目录运行：

```text
RUSTFLAGS=-D warnings cargo fmt --all -- --check
RUSTFLAGS=-D warnings cargo check -p silex_i18n
RUSTFLAGS=-D warnings cargo test -p silex_i18n --test docs_examples
zola --root docs check
```

检查可选的 JSON、持久化和宏入口：

```text
RUSTFLAGS=-D warnings cargo check -p silex_i18n --features json,persist,macros
RUSTFLAGS=-D warnings cargo test -p silex_i18n --features macros --test typed_keys
RUSTFLAGS=-D warnings cargo test -p silex_i18n_macros
```

浏览器相关变更需要目标和 runner：

```text
RUSTFLAGS=-D warnings cargo test -p silex_i18n --features browser-tests --test wasm \
    --target wasm32-unknown-unknown --no-run
RUSTFLAGS=-D warnings cargo test -p silex_i18n --features browser-tests --test browser \
    --target wasm32-unknown-unknown --no-run
```

环境存在浏览器时可以去掉 `--no-run`，交给仓库 `.cargo/config.toml` 的
`wasm-bindgen-test-runner` 执行。没有浏览器时，至少保留 wasm 编译检查，不要把
native 通过误认为 browser metadata/resource 行为已验证。

只修改文档或 `docs/examples/silex_i18n/` 时，不需要运行 workspace 或其它 crate
的测试；至少运行上面的目标 crate 文档示例编译/测试和 `zola --root docs check`。

## 契约清单

修改 locale/catalog 时，至少覆盖：

- 下划线转换、script/region 大小写和 invalid locale error；
- 当前 locale 与 fallback locale 的逐级查找、重复候选去重；
- 缺 key、缺 argument、文本 placeholder、复数 `other` 和 fallback catalog 的 plural rule；
- JSON nested flatten、message/object collision 和未知 plural category。

修改 store/revision 时，至少覆盖：

- `I18nBuilder` 初始 locale 优先级和无 locale 时的 `en` 默认值；
- `t!` 的参数 tracked 读取、locale/fallback/catalog revision 失效；
- equal catalog 不增加 revision，replacement/removal 会更新已有 memo；
- handler、owner close 和 foreign runtime 的错误路径。

修改 `CatalogResource` 时，至少覆盖：

- cache hit 不调用 loader，`refetch` 使用 cache，`reload` 绕过 cache；
- loader locale mismatch、旧 completion 丢弃、Ready/Error 状态和 suspense 计数；
- owner close 后 future、completion 和 catalog 写回不会继续调用用户代码。

修改 browser/persist/Intl 时，至少覆盖：

- exact/language/fallback browser locale 选择和 invalid navigator 值；
- metadata 的嵌套 owner stack、外部修改保护和 owner cleanup restore；
- locale binding 的双向同步、相等值抑制和跨 runtime 拒绝；
- native/wasm Intl 的错误边界，尤其是非有限数字和 timestamp。

## 调试顺序

1. 先把 `Locale` 打印为 `as_str()`，确认规范化后的值，而不是只看输入字符串。
2. 对同步翻译调用 `has_catalog`，再检查当前/fallback chain 中实际存在的 key。
3. 检查 `Catalog::get` 的 `Message` 类型和 placeholder 名称；不要先改 missing policy 来掩盖资源错误。
4. 异步场景区分 cache hit、`refetch`、`reload`，读取 `CatalogResource::state()` 和 loader error。
5. DOM 场景区分 translated text 没更新、`lang/dir` 没同步和 owner 已关闭三类问题。
6. 遇到 `RuntimeMismatch` 或 `NoSuchNode`，检查 source、handler、suspense、binding 的 owner/runtime provenance。

## 对应源码

- 文档示例：`docs/examples/silex_i18n/basic.rs`
- 文档示例测试：`crates/silex_i18n/tests/docs_examples.rs`
- browser/resource 测试：`crates/silex_i18n/tests/browser.rs`、`tests/wasm.rs`
- UI 测试：`crates/silex_i18n/tests/compile_fail.rs`、`tests/ui/`
