+++
title = "如何进行 Wasm 测试"
description = "使用 wasm-bindgen-test-runner、geckodriver 和 Firefox 运行 Wasm 测试。"
weight = 20
+++

# 如何进行 Wasm 测试

本文只介绍 Wasm 测试的执行方式。所有 Cargo 编译和测试命令都使用
`RUSTFLAGS='-D warnings'`，确保警告被视为错误。

## 测试层次

以下命令逐层增加验证范围，不能互相替代：

| 命令 | 能证明什么 | 是否启动 Firefox |
| --- | --- | --- |
| `cargo check --target wasm32-unknown-unknown` | Wasm 目标可以编译 | 否 |
| `cargo test --target wasm32-unknown-unknown --no-run` | Wasm 测试二进制可以生成 | 否 |
| `cargo test --target wasm32-unknown-unknown --test ...` | 指定 test target 在真实 Firefox 中通过 | 是 |
| `cargo +nightly wasm-test-nightly ... -- --include-ignored` | nightly `build-std` 下的 panic unwind 测试通过 | 是 |

只有第三、第四类命令是浏览器验收。`--no-run` 不能替代浏览器运行。

## 准备依赖

在仓库根目录执行：

```sh
rustup target add wasm32-unknown-unknown

command -v firefox
command -v geckodriver
command -v wasm-bindgen-test-runner
```

稳定浏览器测试需要以下四项：

- `wasm32-unknown-unknown`；
- Firefox；
- geckodriver；
- `wasm-bindgen-test-runner`。

如果当前 Shell 设置了 HTTP 代理，让本地 WebDriver 请求绕过代理：

```sh
export NO_PROXY="127.0.0.1,localhost"
export no_proxy="$NO_PROXY"
```

`wasm-bindgen-test-runner` 的常用配置如下：

- `WASM_BINDGEN_USE_BROWSER=1`：使用浏览器测试模式；
- `GECKODRIVER=/absolute/path/to/geckodriver`：让 runner 启动本地驱动；
- `GECKODRIVER_REMOTE=http://127.0.0.1:4444`：连接已经启动的驱动；
- `NO_HEADLESS=1`：关闭 headless，显示浏览器窗口；
- `WASM_BINDGEN_TEST_NO_STREAM=1`：等待测试完成后一次性读取输出。

`GECKODRIVER_HEADLESS=1` 不是当前 runner 使用的配置项。默认情况下 runner
会为 Firefox 加入 `-headless`。

## 启动 WebDriver

推荐在单独终端启动 geckodriver：

```sh
geckodriver --port 4444 --log info
```

保持该终端运行，在另一个终端设置远程驱动后执行测试：

```sh
NO_PROXY=127.0.0.1,localhost \
no_proxy=127.0.0.1,localhost \
GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
WASM_BINDGEN_USE_BROWSER=1 \
RUSTFLAGS='-D warnings' \
cargo test -p silex_dom \
  --target wasm32-unknown-unknown \
  --test host_resources \
  -- --nocapture \
  --skip element_listener_panic_closes_destination_before_owner_cleanup
```

使用 `GECKODRIVER_REMOTE` 时，runner 不会负责启动或终止 geckodriver。测试完成
后在第一个终端按 `Ctrl-C` 停止驱动。

如果需要由 runner 管理驱动，可以使用绝对路径：

```sh
NO_PROXY=127.0.0.1,localhost \
no_proxy=127.0.0.1,localhost \
GECKODRIVER=/absolute/path/to/geckodriver \
WASM_BINDGEN_USE_BROWSER=1 \
RUSTFLAGS='-D warnings' \
cargo test -p silex_dom \
  --target wasm32-unknown-unknown \
  --test host_resources \
  -- --nocapture \
  --skip element_listener_panic_closes_destination_before_owner_cleanup
```

如果 runner 启动驱动时出现连接竞态，可开启日志：

```sh
RUST_LOG=debug \
GECKODRIVER=/absolute/path/to/geckodriver \
GECKODRIVER_ARGS='--log trace' \
RUSTFLAGS='-D warnings' \
cargo test -p silex_dom \
  --target wasm32-unknown-unknown \
  --test host_resources \
  -- --nocapture \
  --skip element_listener_panic_closes_destination_before_owner_cleanup
```

## stable 工具链测试

### 只检查 Wasm 编译

```sh
RUSTFLAGS='-D warnings' \
cargo check -p silex_dom \
  --target wasm32-unknown-unknown \
  --all-targets

RUSTFLAGS='-D warnings' \
cargo test -p silex_dom \
  --target wasm32-unknown-unknown \
  --test host_resources \
  --no-run
```

上述命令不会启动 Firefox。

### 运行浏览器测试

启动独立 geckodriver 后，运行单个 test target：

```sh
NO_PROXY=127.0.0.1,localhost \
no_proxy=127.0.0.1,localhost \
GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
RUSTFLAGS='-D warnings' \
cargo test -p silex_dom \
  --target wasm32-unknown-unknown \
  --test owner \
  -- --nocapture
```

替换 `-p` 后的包名和 `--test` 后的 target 名称即可运行其他 browser test。
例如：

```sh
GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
RUSTFLAGS='-D warnings' \
cargo test -p silex_css \
  --target wasm32-unknown-unknown \
  --test fallback \
  --features test-style-fallback \
  -- --nocapture

GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
RUSTFLAGS='-D warnings' \
cargo test -p silex_bootstrap \
  --target wasm32-unknown-unknown \
  --test app_host \
  -- --nocapture
```

### 稳定 Wasm 的 panic 测试

稳定预编译 Wasm 标准库下，故意触发 Rust panic 的测试可能输出
`RuntimeError: unreachable executed` 并使 target 失败。这不是 WebDriver
启动失败。

只验证其余测试时，使用 `--skip`：

```sh
GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
RUSTFLAGS='-D warnings' \
cargo test -p silex_dom \
  --target wasm32-unknown-unknown \
  --test host_resources \
  -- --nocapture \
  --skip element_listener_panic_closes_destination_before_owner_cleanup
```

需要验证 panic 捕获、cleanup 或 rollback 时，使用下一节的 nightly 命令。

## nightly `build-std` 测试

stable `wasm32-unknown-unknown` 使用预编译标准库。需要验证 Wasm unwind 时，
使用 nightly、`rust-src` 和仓库提供的 `build-std` alias：

```sh
rustup component add rust-src --toolchain nightly
```

设置本次 Shell 使用的 flags：

```sh
export RUSTFLAGS='-D warnings -Cpanic=unwind -Cllvm-args=-wasm-use-legacy-eh=false'
```

`-Cpanic=unwind` 启用 Rust unwind；
`-Cllvm-args=-wasm-use-legacy-eh=false` 避免生成 Firefox 无法验证的 legacy
exception 指令。不要把这组 flags 写入系统级配置。

先编译，再运行浏览器测试：

```sh
cargo +nightly wasm-check-nightly -p silex_dom --all-targets

NO_PROXY=127.0.0.1,localhost \
no_proxy=127.0.0.1,localhost \
GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
WASM_BINDGEN_USE_BROWSER=1 \
cargo +nightly wasm-test-nightly \
  -p silex_dom \
  --test host_resources \
  -- --include-ignored --nocapture
```

`--include-ignored` 用于执行被标记为 `#[ignore]` 的 unwind 测试。nightly
alias 会重新编译 `core`、`std`、`alloc` 等标准库组件，首次构建会明显慢于
stable 路径。

测试完成后恢复 Shell 环境：

```sh
unset RUSTFLAGS
```

## 故障排查

### 找不到 WebDriver

错误示例：

```text
failed to find a suitable WebDriver binary
```

检查驱动路径和版本：

```sh
command -v geckodriver
geckodriver --version
```

需要自定义路径时，为 `GECKODRIVER` 设置绝对路径。

### `Peer disconnected` 或 `Connection reset by peer`

优先使用独立 geckodriver，并确认本地请求绕过代理：

```sh
NO_PROXY=127.0.0.1,localhost \
no_proxy=127.0.0.1,localhost \
GECKODRIVER_REMOTE=http://127.0.0.1:4444 \
cargo test -p silex_dom --target wasm32-unknown-unknown --test owner
```

如果仍然失败，查看 geckodriver 的 `--log trace` 输出，确认是否收到
`POST /session`，以及 Firefox 是否成功连接 Marionette。

### `signal: 9 (SIGKILL)`

runner 退出清理阶段主动终止自己启动的 geckodriver 时，可能显示该状态。不能
单独据此判断 Firefox 被 OOM 杀死；先查看它之前的 `Peer disconnected` 或
`Connection reset by peer`。

### `Operation not permitted` 或无法监听 `127.0.0.1`

这是运行环境禁止本地回环监听，不是 Rust 测试失败。需要在允许本地监听和启动
Firefox 的环境中运行浏览器测试；`--no-run` 仍可用于验证 Wasm 测试二进制生成。

### `RuntimeError: unreachable executed`

先确认失败测试是否故意触发 panic。稳定 Wasm 测试下，panic 可能使 test target
失败；不要把该输出直接归因于 DOM 或 WebDriver。需要验证 panic unwind 时，改用
nightly `build-std` 命令。

### 测试输出不完整

为 Cargo 测试参数加入 `--nocapture`：

```sh
cargo test -p silex_dom --target wasm32-unknown-unknown --test owner -- --nocapture
```

如果仍然需要一次性读取 runner 输出，可设置：

```sh
WASM_BINDGEN_TEST_NO_STREAM=1 cargo test ...
```
