//! 运行时与宿主环境之间的两处接缝：诊断输出与微任务调度。
//!
//! 抽出这一层的理由与 `backend.rs` 相同——`registry.rs` / `dynamic.rs` 里真正
//! 容易改坏的是状态机，而状态机的每一步收尾都夹着一次 `spawn_local`：
//! 「借不到注册表 → 排队 → 下一个微任务补做」。`wasm_bindgen_futures::spawn_local`
//! 在非 wasm 目标上编译得过、一调用就 panic，于是这条补做路径在原生测试里根本
//! 走不到。把「排到下一个微任务」变成一个接缝之后，测试可以自己决定什么时候把
//! 队列跑干净，从而精确控制时序。

/// 运行时异常的统一出口。
///
/// 样式注入失败、清理失败这类问题此前一律 `let _ = …` 吞掉，症状是「样式莫名
/// 不生效」而没有任何线索。wasm 的 debug 构建下打到 `console.error`，release 下
/// 不产生任何代码；非 wasm 下记进一个环形缓冲，测试可以断言「确实报了这一条」。
#[inline]
pub(crate) fn report(what: &str) {
    imp::report(what);
}

/// 把一段收尾工作排到下一个微任务。
#[inline]
pub(crate) fn schedule_microtask(f: impl FnOnce() + 'static) {
    imp::schedule_microtask(f);
}

#[cfg(target_arch = "wasm32")]
mod imp {
    #[inline]
    pub(super) fn report(_what: &str) {
        #[cfg(debug_assertions)]
        web_sys::console::error_1(&format!("[silex-css] {}", _what).into());
    }

    #[inline]
    pub(super) fn schedule_microtask(f: impl FnOnce() + 'static) {
        wasm_bindgen_futures::spawn_local(async move { f() });
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::{cell::RefCell, collections::VecDeque};

    /// 非测试构建里没人读这些记录，但记录本身要一直在——否则测试与非测试是
    /// 两条不同的代码路径，测出来的东西不作数。缓冲有上限，不会无限增长。
    const REPORT_LIMIT: usize = 64;

    thread_local! {
        static REPORTS: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
        static MICROTASKS: RefCell<VecDeque<Box<dyn FnOnce()>>> = const { RefCell::new(VecDeque::new()) };
    }

    // 这两条会被 `Drop` 走到，而 `Drop` 可能发生在线程退出时的 TLS 析构里，那时
    // 别的 TLS 可能已经没了：一律 `try_with`，做不了就算了——TLS 析构器里 panic
    // 会直接 abort 进程。
    pub(super) fn report(what: &str) {
        let _ = REPORTS.try_with(|r| {
            let mut r = r.borrow_mut();
            if r.len() == REPORT_LIMIT {
                r.pop_front();
            }
            r.push_back(what.to_string());
        });
    }

    pub(super) fn schedule_microtask(f: impl FnOnce() + 'static) {
        let _ = MICROTASKS.try_with(|q| q.borrow_mut().push_back(Box::new(f)));
    }

    /// 取走至今为止的全部诊断输出。
    #[cfg(test)]
    pub(crate) fn take_reports() -> Vec<String> {
        REPORTS.with(|r| r.borrow_mut().drain(..).collect())
    }

    /// 把微任务队列跑干净——包括跑的过程中新排进来的那些。返回执行了多少个。
    #[cfg(test)]
    pub(crate) fn run_microtasks() -> usize {
        let mut ran = 0usize;
        // 借用不能跨越 `task()`：任务自己还会往队列里排东西
        while let Some(task) = MICROTASKS.with(|q| q.borrow_mut().pop_front()) {
            task();
            ran += 1;
            assert!(ran < 10_000, "微任务队列没有收敛");
        }
        ran
    }

    #[cfg(test)]
    pub(crate) fn reset() {
        REPORTS.with(|r| r.borrow_mut().clear());
        MICROTASKS.with(|q| q.borrow_mut().clear());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use imp::{reset, run_microtasks, take_reports};
