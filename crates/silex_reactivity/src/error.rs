//! Explicit runtime operation errors.

use std::fmt;

/// A response to an operation that cannot be completed in the current scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactiveError {
    /// The handle's scope has already ended or the node was disposed.
    NoSuchNode,
    /// A typed operation reached a node of another internal kind.
    WrongKind,
    /// The node contains a different Rust type than the operation requested.
    TypeMismatch,
    /// A read or write lease conflicts with another lease on the same node.
    BorrowConflict,
    /// A computation or callback is being entered recursively.
    Reentrant,
    /// A runtime was asked to start another run while it was already running.
    RuntimeAlreadyRunning,
    /// Reactive nodes from different scheduler families were combined.
    RuntimeMismatch,
}

impl ReactiveError {
    #[inline]
    pub fn is_bug(self) -> bool {
        matches!(self, Self::WrongKind | Self::TypeMismatch | Self::Reentrant)
    }
}

impl fmt::Display for ReactiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoSuchNode => "节点不存在或所属 scope 已结束",
            Self::WrongKind => "句柄指向的节点不是这个操作要求的种类",
            Self::TypeMismatch => "节点里存放的不是请求的类型",
            Self::BorrowConflict => "同一节点上的动态借用发生冲突",
            Self::Reentrant => "响应式计算或回调发生递归调用",
            Self::RuntimeAlreadyRunning => "响应式 Runtime 已经在运行中",
            Self::RuntimeMismatch => "响应式节点属于不同的 Runtime scheduler family",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ReactiveError {}

pub type ReactiveResult<T> = Result<T, ReactiveError>;
