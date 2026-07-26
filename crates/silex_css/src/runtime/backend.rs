//! 「对样式表做什么」与「样式表是什么」的分界。
//!
//! 抽出这一层不是为了多态——运行时只会有一个实现在用——而是为了让上面那层
//! 状态机（退休 LRU、延迟队列、`adoptedStyleSheets` 的增删时序）能脱离浏览器
//! 被断言。`runtime/dynamic.rs` 与 `runtime/registry.rs` 里真正容易改坏的就是
//! 这层状态机：它决定哪张表进文档、什么时候进、退休之后还能不能捡回来，而这些
//! 判断没有一条需要浏览器在场。
//!
//! 分界之后 `ActiveSheet` 是 type alias 而不是 `dyn`，静态分发，没有运行时开销。

use core::fmt::Debug;

/// 一张样式表能被做的事。
pub(crate) trait SheetBackend: Sized + 'static {
    /// 表的句柄——参与 `adoptedStyleSheets` 的那个身份。
    ///
    /// wasm 下是 `CssStyleSheet`（`PartialEq` 即 JS 对象标识），测试下是一个
    /// 递增序号。**必须是对象标识而不是 Rust 侧地址**：报告 P2-1 里那次误判
    /// 正是因为拿 `Vec` 元素的内存地址当身份，扩容一次身份就全变了，反过来
    /// 同一微任务内增删数量相等时新元素又可能落回同一批槽位。
    type Handle: Clone + PartialEq + Debug;

    /// 建一张新表。两条路都走不通时返回 `None`（此时调用方应当放弃，而不是
    /// 假装成功）。
    fn create() -> Option<Self>;

    /// 整表替换。
    fn replace(&self, css: &str);

    /// 往表尾追加若干条**顶层规则**。
    ///
    /// 成功返回 `true`；后端没有增量接口、或某条规则被拒时返回 `false`，
    /// 由调用方退回整表替换。
    fn append_rules(&self, rules: &[&str]) -> bool;

    /// 参与 `adoptedStyleSheets` 的句柄（`<style>` 兜底没有）。
    fn adopted(&self) -> Option<Self::Handle>;

    /// 把不参与 `adoptedStyleSheets` 的那种表从文档里摘掉。参与的那些由
    /// `DocumentStyleRegistry` 负责摘除，这里不重复。
    fn detach(&self);
}

/// 文档级的那一次写入：`document.adoptedStyleSheets = [...]`。
pub(crate) trait DocumentBackend<H>: 'static {
    fn set_adopted(sheets: &[H]);
}

#[cfg(target_arch = "wasm32")]
pub(crate) type ActiveSheet = super::sheet::Sheet;
#[cfg(target_arch = "wasm32")]
type ActiveDocument = super::sheet::WebDocument;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type ActiveSheet = super::fake::FakeSheet;
#[cfg(not(target_arch = "wasm32"))]
type ActiveDocument = super::fake::FakeDocument;

/// 当前后端的表句柄。
pub(crate) type SheetHandle = <ActiveSheet as SheetBackend>::Handle;

/// 把这一批表写进文档。
pub(crate) fn set_adopted(sheets: &[SheetHandle]) {
    <ActiveDocument as DocumentBackend<SheetHandle>>::set_adopted(sheets);
}
