//! 统一的失败语义。
//!
//! 从前每条路径各报各的：`try_update_signal` 返回一个 5 变体的 `UpdateOutcome`，
//! `try_update_signal_silent` / `try_with_signal` / `try_update_stored_value`
//! 把**四种截然不同的失败**（没有运行时 / 节点不存在 / 种类不对 / 类型不符 / 重入）
//! 压成一个 `None`，而 `update_signal` 干脆返回 `()` —— debug 下断言、release 下
//! 静默丢弃（审计报告 §3.2）。调用方因此无法区分“这个句柄失效了（该报错）”
//! 和“类型写错了（编程 bug）”。
//!
//! 现在所有 `try_*` 一律返回 [`ReactiveResult`]。

use std::fmt;

/// 一次响应式操作可能的失败原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactiveError {
    /// 当前线程还没有运行时 —— 也就意味着不可能有任何节点。
    ///
    /// 只读、或只写既有节点的路径不会仅仅为了报告失败而把运行时建起来
    /// （AUDIT P19.9），因此这条和 [`NoSuchNode`](Self::NoSuchNode) 是分开的。
    NoRuntime,
    /// 节点不存在或已被销毁。**这是调用方常见且合法的情况**（句柄是 `Copy` 的，
    /// 它的存活与节点无关）。
    NoSuchNode,
    /// 节点还活着，但不是这个操作要求的种类 —— 例如拿一个 stored value 的句柄
    /// 去读 signal。
    ///
    /// 带种类的句柄（[`Handle<K>`](crate::Handle)）已经在编译期挡掉了绝大多数
    /// 这类错误，因此它现在基本只出现在用 [`RawId`](crate::RawId)
    /// 显式擦除种类的逃生路径上。
    WrongKind,
    /// 种类对了，但里面存放的不是 `T`。**这是编程错误**，不是运行时状态。
    TypeMismatch,
    /// 值正被外层的 update / with 闭包借出。
    ///
    /// 运行时在把值交给用户闭包之前会把它**移出**节点（节点里暂时是空的），
    /// 这样运行时就不必在用户代码执行期间持有指向节点的引用（AUDIT P5、
    /// 审计报告 §2.1）。代价就是这条契约：不允许在闭包内访问同一个节点。
    Reentrant,
}

impl ReactiveError {
    /// 这个失败是不是编程错误（而不是“节点恰好没了”这种正常的运行时状态）。
    ///
    /// [`update`](crate::signal::update) 这类不返回错误的便捷函数用它决定
    /// 要不要在 debug 构建下断言。
    #[inline(always)]
    pub fn is_bug(self) -> bool {
        matches!(self, Self::WrongKind | Self::TypeMismatch | Self::Reentrant)
    }
}

impl fmt::Display for ReactiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::NoRuntime => "当前线程还没有响应式运行时",
            Self::NoSuchNode => "节点不存在或已被销毁",
            Self::WrongKind => "句柄指向的节点不是这个操作要求的种类",
            Self::TypeMismatch => "节点里存放的不是请求的类型",
            Self::Reentrant => "值正被外层闭包借出，不允许在闭包内访问同一个节点",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ReactiveError {}

/// 所有 `try_*` 的返回类型。
pub type ReactiveResult<T> = Result<T, ReactiveError>;
