+++
title = "测试与调试"
description = "silex_bootstrap 的 native、wasm、编译期 scope 和文档示例验证方法。"
weight = 50
+++

# 测试与调试

`silex_bootstrap` 的契约分布在 Rust 状态机、`silex_dom` 的 mount boundary、
浏览器事件和 wasm-bindgen JavaScript 对象之间。只验证一个 view 能挂载，无法
证明 replace rollback、listener ownership 或 scope escape 正确，因此测试按
平台和边界分层。

## 测试分层

| 位置 | 覆盖内容 | 环境 |
| --- | --- | --- |
| `tests/error.rs` | `AppHostError` 对 mount/dispose report 的结构化保留。 | native |
| `tests/compile_fail.rs` + `tests/ui/` | builder 中的 `OwnerAccess` 不能逃逸到 `'static`。 | native trybuild |
| `tests/docs_examples.rs` | `docs/examples/silex_bootstrap/basic.rs` 的编译与执行。 | native + wasm |
| `tests/app_host.rs` | host 状态、mount/retry、replace、Drop、外部移除 target。 | wasm browser |
| `tests/page_controller.rs` | page policy、事件过滤、listener drop、reentrancy 和 reporter。 | wasm browser |
| `tests/browser_bootstrap.rs` | id target 解析、缺失 target、policy 和 JS transfer。 | wasm browser + feature |
| `tests/js_object.rs` | `JsAppHost` owner、state、幂等 unmount。 | wasm browser + feature |
| `src/js_object.rs` 单元/wasm tests | JS error object shape、fatal/recoverable 字段和 unwind safety。 | native + wasm |

browser 集成测试通过 `#![cfg(target_arch = "wasm32")]` 隔离，且使用
`wasm_bindgen_test_configure!(run_in_browser)`。native `cargo test` 会编译
这些 target-specific 文件但不会执行 browser case；feature-gated 测试只有
相应 feature 开启时才有内容。

## 常用验证命令

只修改本 crate 和其文档时，在仓库根目录执行：

```text
cargo fmt --all -- --check
RUSTFLAGS=-Dwarnings cargo check -p silex_bootstrap
RUSTFLAGS=-Dwarnings cargo test -p silex_bootstrap --test docs_examples
RUSTFLAGS=-Dwarnings cargo test -p silex_bootstrap --test error
RUSTFLAGS=-Dwarnings cargo test -p silex_bootstrap --test compile_fail
zola --root docs check
```

新增或修改文档示例后，`--test docs_examples` 是最小必跑检查。要编译示例
中真正的 browser 分支，可使用：

```text
RUSTFLAGS=-Dwarnings cargo test -p silex_bootstrap --test docs_examples \
    --target wasm32-unknown-unknown --no-run
```

如果要检查可选 API 的 wasm 编译而不启动浏览器：

```text
RUSTFLAGS=-Dwarnings cargo check -p silex_bootstrap --all-features \
    --target wasm32-unknown-unknown
```

具备 browser runner 时，再按 feature 运行 `app_host`、`page_controller`、
`browser_bootstrap` 和 `js_object`；没有 browser 时至少保留 wasm `--no-run`
编译检查。无需为了本 crate 文档运行 workspace 或其它 crate 的测试。

## 行为契约清单

修改 `AppHost` 时，至少覆盖：

- `Ready` host 拒绝第二次 `mount`，而 `replace` 只接受 `Active`；
- builder clean failure 会保留 primary error、清空 boundary 并允许 retry；
- rollback cleanup failure、dispose failure、panic 和 reentry 会 poison；
- replace 先 dispose 旧 app，旧清理失败时不会调用新 builder；
- unmount 无 active app 返回 `AlreadyUnmounted`，重复调用不增加清理；
- target 被外部移除后，host 仍能 dispose owner 和 boundary 中的节点；
- Drop 只把不能返回的失败交给 `CleanupSink`。

修改 page controller 时，至少覆盖：

- `Manual` 不安装 listener，`PageHide` 只监听 `pagehide`；
- visibility policy 在 document visible 时忽略事件，在 hidden 时才 unmount；
- 安装新 policy 前移除旧 listeners，失败不会留下旧 policy；
- controller drop 先清理 listener，再释放内部 host；
- listener callback 遇到 host borrow conflict 时通过 reporter 报告
  `ReentrantOperation`。

修改 JavaScript boundary 时，至少覆盖：

- `JsAppHost` 只由 Rust transfer 创建，drop 只释放一次 boxed host；
- JS `unmount` 对 `Disposed` 与 `AlreadyUnmounted` 都成功；
- AppHost/Bootstrap 错误的 code、primary、rollback/report 字段稳定；
- fatal/recoverable strategy 与 error kind 不混为一个字符串；
- BrowserBootstrap 在 non-Manual policy 下拒绝 transfer。

## 编译期 scope 契约

`tests/ui/fail_app_host_scope_escape.rs` 将 `ctx.access()` 写入
`Option<OwnerAccess<'static>>`，应保持编译失败；`pass_app_host_builder.rs`
验证合法 HRTB builder 仍能作为 helper 的参数传递。

修改 `AppHost::mount`、`MountContext` 或 error handler 的 lifetime/signature
时，应先调整最小 trybuild case，再检查 `.stderr`。不要用放宽 lifetime 或
删除 `for<'scope>` 的方式让用例通过；这会允许 owner-bound 能力越过 root
session 的清理边界。

## 调试顺序

1. 先读 `host.state()` 和 `is_active()`，区分“没有 active app”和“target
   被外部从 document 移除”；后者不会自动改变 host 状态。
2. 对 mount 失败读取 `mount_error().primary()`、`rollback()` 和
   `availability()`；不要只看 `AppHostError` 的 Display。
3. 对 dispose 失败读取 `dispose_error().report()`，按 cleanup origin 与
   boundary error 分离 owner 清理和节点移除问题。
4. 对 page event 检查当前 policy、document.hidden、listener 是否已被
   `remove_page_lifecycle` 清空，并在 reporter 中记录 `BootstrapError`。
5. 对 JS 错误先按 `code` 分支，再检查 `primary`、`rollback`、`report` 和
   `diagnostic`；不要把结构化报告重新压平成一条日志。
6. 对 scope 编译失败只修改最小 UI case，确认 builder callback 没有把
   `MountContext`、handler 或 owner token 保存到更长生命周期。

## 文档示例

页面通过 `load_data(..., format="plain")` 读取
`docs/examples/silex_bootstrap/basic.rs`，测试入口保持为：

```rust
#[path = "../../../docs/examples/silex_bootstrap/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented bootstrap example should compile and run");
}
```

wasm 分支使用 `wasm-bindgen-test` 执行实际 DOM mount/unmount；native 分支不
访问 window/document，只保证 crate 的文档入口在无浏览器构建中成立。
