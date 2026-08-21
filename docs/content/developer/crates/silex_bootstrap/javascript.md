+++
title = "JavaScript owner 边界"
description = "JsAppHost 的 wasm-bindgen API、结构化错误对象和 raw pointer 所有权不变量。"
weight = 40
+++

# JavaScript owner 边界

启用 `js-object` 后，`silex_bootstrap` 提供 `JsAppHost`，用于把一个已经在
Rust 侧构造并挂载的 `AppHost` 转移给 JavaScript。它是一个故意很小的
wasm-bindgen wrapper：Rust 负责创建 runtime、构造 view 和决定 mount 失败
策略，JavaScript 只读取状态、判断 active 并请求 unmount。

## Feature 和 transfer 前提

`js-object` feature 增加 `wasm-bindgen`、`js-sys` 依赖，并导出：

- `JsAppHost::from_app_host(host)`：消费 `AppHost`，创建 JavaScript-facing owner；
- `JsAppHost::is_active()`：返回 `Result<bool, JsValue>`；
- `JsAppHost::state()`：返回稳定的小写状态字符串；
- `JsAppHost::unmount()`：把 Rust 的 dispose 结果转换为 `Result<(), JsValue>`；
- `bootstrap_error_to_js(&BootstrapError)`：把 bootstrap 错误转换为结构化 JS object。

`JsAppHost` 没有通用的 `mount` 或 `replace` 方法。这是 API 边界的一部分：
挂载 builder 包含 Rust lifetime、`MountContext` 和错误 handler，不能让一个
任意 JavaScript callback 伪造这些 owner-bound 能力。

使用 `BrowserBootstrap` 时，必须先移除页面 policy，再调用
`BrowserBootstrap::into_js_host()`；否则 page listener 仍由 Rust controller
拥有，转移会返回 `BootstrapError::Lifecycle`。直接使用 `from_app_host` 时，
调用方同样必须确保 host 不再被其它 Rust owner 使用。

## JavaScript API

| 方法 | 成功值 | 失败值 |
| --- | --- | --- |
| `is_active()` | 当前 host 是否有 active root/session。 | `JsValue` 字符串，来源于 `SilexError`。 |
| `state()` | `"ready"`、`"mounting"`、`"active"`、`"disposing"` 或 `"poisoned"`。 | 不返回 `Result`。 |
| `unmount()` | `undefined`；`Disposed` 与 `AlreadyUnmounted` 都成功。 | 结构化 AppHost 错误对象或 panic 对象。 |

`unmount` 的幂等性是有意的。JavaScript 不需要先根据 `state()` 竞争式判断
是否已经清理；但如果返回错误，仍应保留对象中的 `code`、`rollback` 和
`report`，不要只显示 `message`。

## 结构化错误对象

`bootstrap_error_to_js` 和 `JsAppHost::unmount` 产生的对象保持稳定字段，
以便 JavaScript adapter 按 code 分支处理：

| Rust 错误 | `code` | 其它字段 |
| --- | --- | --- |
| `AlreadyMounted` | `already-mounted` | 无额外字段。 |
| `NotMounted` | `not-mounted` | 无额外字段。 |
| `InvalidState` | `invalid-state` | `state` 为小写 host state。 |
| `Mount` | `mount` | `primary`、`rollback`。 |
| `Dispose` | `dispose` | `rollback` 与 `report`，两者是同一报告形状。 |
| `ReentrantOperation` | `reentrant` | 无额外字段。 |
| `Poisoned` | `poisoned` | 无额外字段。 |
| `TargetNotFound(id)` | `target-not-found` | `target` 为原始 id。 |
| `Lifecycle` | `lifecycle` | 无额外字段。 |
| `Listener` | `listener` | 无额外字段。 |

所有 bootstrap/app-host 错误对象还包含 `message`、`primary` 和 `rollback`；
没有对应报告时后两个字段是 `undefined`。`Mount` 的 `primary` 形状为：

```text
{
  strategy: "recoverable" | "fatal",
  kind: "dom" | "framework" | "mount" | ...,
  message: string
}
```

`rollback`/`report` 的形状为：

```text
{
  clean: boolean,
  cleanupFailures: Array<{
    origin: "root" | "provisional-owner" | "mount-boundary",
    diagnostic: {
      message: string,
      payloadKind: "string" | "static-str" | "unknown"
    }
  }>,
  boundaryErrors: Array<{ strategy: string, kind: string, message: string }>
}
```

当 `JsAppHost::unmount` 捕获 unwind panic 时，返回的对象使用
`code: "panic"`，并增加 `state: "poisoned"`、`operation: "unmount"` 和
`diagnostic`。Rust panic 策略为 abort 时不能依赖这个转换路径；目标不会
先执行 Rust 的 panic unwinding。

## raw pointer 不变量

`JsAppHost` 内部把 `Box<AppHost>` 转为 `usize`，让 wasm-bindgen receiver
保持简单，再在 Rust 方法中恢复共享或独占引用。这个 `unsafe` 边界依赖以下
不变量：

1. 只有 `from_app_host` 创建 pointer，且创建后只有一个 wrapper 拥有它；
2. `host()` 只在 wrapper 仍然拥有 pointer 时构造共享引用，`host_mut()` 只
   在没有其它 Rust 使用者时构造独占引用；
3. `Drop` 只调用一次 `Box::from_raw`，释放后不再使用 pointer；
4. `Drop` 用 `catch_unwind` 隔离 host 清理 panic，并在 sink/console 诊断失败
   时避免让 wrapper drop 再次传播 panic。

因此不要复制、手工构造或缓存 `JsAppHost` 的内部整数；不要在 transfer 后
继续使用原 `AppHost`。`from_app_host` 是所有权转移，不是借用注册。

## 对应测试

- `tests/js_object.rs`：active owner 保持、state 字符串和幂等 unmount。
- `src/js_object.rs` 的 wasm tests：AppHost 错误对象字段、`strategy` 与
  `kind` 分离，以及 `RefUnwindSafe` 约束。
- `tests/browser_bootstrap.rs`：BrowserBootstrap 不能在仍持有 page listener
  时隐式转移给 JS。
