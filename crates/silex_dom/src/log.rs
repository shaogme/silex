//! 同构日志记录工具。
//!
//! 浏览器目标写入 `web_sys::console`；native 和 SSR 目标写入标准输出或标准
//! 错误输出。日志模块不依赖 `silex_core`，因此可以被 DOM 错误和 browser
//! bridge 共同使用。

/// 使用 `println!()` 风格的格式化记录普通日志。
#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {
        $crate::log::console_log(format_args!($($t)*).to_string())
    };
}

/// 使用 `println!()` 风格的格式化记录警告日志。
#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => {
        $crate::log::console_warn(format_args!($($t)*).to_string())
    };
}

/// 使用 `println!()` 风格的格式化记录错误日志。
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {
        $crate::log::console_error(format_args!($($t)*).to_string())
    };
}

/// 仅在 debug 构建中记录普通日志。
#[macro_export]
macro_rules! debug_log {
    ($($t:tt)*) => {
        $crate::log::console_debug_log(format_args!($($t)*).to_string())
    };
}

/// 仅在 debug 构建中记录警告日志。
#[macro_export]
macro_rules! debug_warn {
    ($($t:tt)*) => {
        $crate::log::console_debug_warn(format_args!($($t)*).to_string())
    };
}

/// 仅在 debug 构建中记录错误日志。
#[macro_export]
macro_rules! debug_error {
    ($($t:tt)*) => {
        $crate::log::console_debug_error(format_args!($($t)*).to_string())
    };
}

const fn writes_stdout() -> bool {
    cfg!(not(all(
        target_arch = "wasm32",
        not(any(target_os = "emscripten", target_os = "wasi"))
    )))
}

/// 记录普通日志。
pub fn console_log<S: AsRef<str>>(message: S) {
    #[allow(clippy::print_stdout)]
    if writes_stdout() {
        println!("{}", message.as_ref());
    } else {
        browser_console_log(message.as_ref());
    }
}

/// 记录警告日志。
pub fn console_warn<S: AsRef<str>>(message: S) {
    if writes_stdout() {
        eprintln!("{}", message.as_ref());
    } else {
        browser_console_warn(message.as_ref());
    }
}

/// 记录错误日志。
#[inline(always)]
pub fn console_error<S: AsRef<str>>(message: S) {
    if writes_stdout() {
        eprintln!("{}", message.as_ref());
    } else {
        browser_console_error(message.as_ref());
    }
}

/// 仅在 debug 构建中记录普通日志。
#[inline(always)]
pub fn console_debug_log<S: AsRef<str>>(message: S) {
    if cfg!(debug_assertions) {
        console_log(message);
    }
}

/// 仅在 debug 构建中记录警告日志。
#[inline(always)]
pub fn console_debug_warn<S: AsRef<str>>(message: S) {
    if cfg!(debug_assertions) {
        console_warn(message);
    }
}

/// 仅在 debug 构建中记录错误日志。
#[inline(always)]
pub fn console_debug_error<S: AsRef<str>>(message: S) {
    if cfg!(debug_assertions) {
        console_error(message);
    }
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "browser",
    not(any(target_os = "emscripten", target_os = "wasi"))
))]
fn browser_console_log(message: &str) {
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(message));
}

#[cfg(not(all(
    target_arch = "wasm32",
    feature = "browser",
    not(any(target_os = "emscripten", target_os = "wasi"))
)))]
fn browser_console_log(_message: &str) {}

#[cfg(all(
    target_arch = "wasm32",
    feature = "browser",
    not(any(target_os = "emscripten", target_os = "wasi"))
))]
fn browser_console_warn(message: &str) {
    web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(message));
}

#[cfg(not(all(
    target_arch = "wasm32",
    feature = "browser",
    not(any(target_os = "emscripten", target_os = "wasi"))
)))]
fn browser_console_warn(_message: &str) {}

#[cfg(all(
    target_arch = "wasm32",
    feature = "browser",
    not(any(target_os = "emscripten", target_os = "wasi"))
))]
fn browser_console_error(message: &str) {
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(message));
}

#[cfg(not(all(
    target_arch = "wasm32",
    feature = "browser",
    not(any(target_os = "emscripten", target_os = "wasi"))
)))]
fn browser_console_error(_message: &str) {}

#[cfg(test)]
mod tests;
