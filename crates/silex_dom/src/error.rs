//! DOM 和宿主清理错误。
//!
//! View mount 状态错误位于 `silex_view::error`；本模块只描述低层 DOM 错误和
//! 清理诊断，避免 DOM backend 反向知道 View 生命周期。

use crate::log::console_error;
use silex_core::{CleanupDiagnostic, CloseError, SilexError};
use std::{fmt, rc::Rc};

/// Structured error for low-level DOM, backend and host operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomError {
    CrossContext {
        expected: u64,
        actual: u64,
    },
    InvalidHandle {
        backend: u64,
        kind: &'static str,
    },
    WrongNodeKind {
        expected: &'static str,
        actual: &'static str,
    },
    CannotContain {
        parent: &'static str,
    },
    Cycle,
    CannotRemoveDocument,
    NoParent,
    ParentMismatch,
    ReferenceNotChild,
    AttributeNameEmpty,
    NodeRefBorrowed,
    Unsupported {
        capability: &'static str,
    },
    Backend {
        operation: &'static str,
        message: String,
    },
}

pub type DomResult<T> = Result<T, DomError>;

impl fmt::Display for DomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrossContext { expected, actual } => write!(
                formatter,
                "DOM handles belong to different contexts (expected {expected}, got {actual})"
            ),
            Self::InvalidHandle { backend, kind } => {
                write!(formatter, "invalid {kind} handle for backend {backend}")
            }
            Self::WrongNodeKind { expected, actual } => {
                write!(formatter, "expected {expected} node, got {actual}")
            }
            Self::CannotContain { parent } => write!(formatter, "{parent} cannot contain children"),
            Self::Cycle => formatter.write_str("the tree operation would create a cycle"),
            Self::CannotRemoveDocument => {
                formatter.write_str("the document node cannot be removed")
            }
            Self::NoParent => formatter.write_str("the node has no parent"),
            Self::ParentMismatch => formatter.write_str("the node does not belong to that parent"),
            Self::ReferenceNotChild => {
                formatter.write_str("the reference is not a child of the parent")
            }
            Self::AttributeNameEmpty => formatter.write_str("attribute or property name is empty"),
            Self::NodeRefBorrowed => formatter.write_str("the DOM NodeRef is already borrowed"),
            Self::Unsupported { capability } => {
                write!(formatter, "unsupported capability: {capability}")
            }
            Self::Backend { operation, message } => {
                write!(formatter, "{operation} failed: {message}")
            }
        }
    }
}

impl std::error::Error for DomError {}

/// 标识清理失败发生在生命周期边界中的位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupOrigin {
    Root,
    ProvisionalOwner,
    MountBoundary,
}

/// 一条带有来源的 owner/DOM 清理失败。
#[derive(Clone, Debug)]
pub struct CleanupFailure {
    pub origin: CleanupOrigin,
    pub error: CloseError,
}

impl CleanupFailure {
    pub fn new(origin: CleanupOrigin, error: CloseError) -> Self {
        Self { origin, error }
    }

    pub fn into_parts(self) -> (CleanupOrigin, CloseError) {
        (self.origin, self.error)
    }
}

/// 一次 rollback 或 dispose 收集到的全部清理和 boundary 错误。
#[derive(Clone, Debug, Default)]
pub struct CleanupReport {
    cleanup_failures: Vec<CleanupFailure>,
    boundary_errors: Vec<SilexError>,
}

impl CleanupReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_parts(
        cleanup_failures: Vec<CleanupFailure>,
        boundary_errors: Vec<SilexError>,
    ) -> Self {
        Self {
            cleanup_failures,
            boundary_errors,
        }
    }

    pub fn is_clean(&self) -> bool {
        self.cleanup_failures.is_empty() && self.boundary_errors.is_empty()
    }

    pub fn cleanup_failures(&self) -> &[CleanupFailure] {
        &self.cleanup_failures
    }

    pub fn boundary_errors(&self) -> &[SilexError] {
        &self.boundary_errors
    }

    pub fn into_parts(self) -> (Vec<CleanupFailure>, Vec<SilexError>) {
        (self.cleanup_failures, self.boundary_errors)
    }
}

/// Drop 阶段将 `CloseError` 转换成的结构化诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupFailureDiagnostic {
    origin: CleanupOrigin,
    diagnostic: CleanupDiagnostic,
}

impl CleanupFailureDiagnostic {
    pub fn new(origin: CleanupOrigin, diagnostic: CleanupDiagnostic) -> Self {
        Self { origin, diagnostic }
    }

    pub fn origin(&self) -> CleanupOrigin {
        self.origin
    }

    pub fn diagnostic(&self) -> &CleanupDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (CleanupOrigin, CleanupDiagnostic) {
        (self.origin, self.diagnostic)
    }
}

/// Drop 期间不会 panic 穿透的清理诊断报告。
#[derive(Debug, Default)]
pub struct DropFailureReport {
    cleanup: Vec<CleanupFailureDiagnostic>,
    boundary: Vec<SilexError>,
}

impl DropFailureReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_parts(cleanup: Vec<CleanupFailureDiagnostic>, boundary: Vec<SilexError>) -> Self {
        Self { cleanup, boundary }
    }

    pub fn is_clean(&self) -> bool {
        self.cleanup.is_empty() && self.boundary.is_empty()
    }

    pub fn cleanup_failures(&self) -> &[CleanupFailureDiagnostic] {
        &self.cleanup
    }

    pub fn boundary_errors(&self) -> &[SilexError] {
        &self.boundary
    }

    pub fn into_parts(self) -> (Vec<CleanupFailureDiagnostic>, Vec<SilexError>) {
        (self.cleanup, self.boundary)
    }
}

/// `'static` 的 Drop 诊断接收器。
#[derive(Clone)]
pub struct CleanupSink {
    callback: Rc<dyn Fn(DropFailureReport)>,
}

impl CleanupSink {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(DropFailureReport) + 'static,
    {
        Self {
            callback: Rc::new(handler),
        }
    }

    pub fn console() -> Self {
        Self::new(|report| {
            console_error(format!("Silex cleanup failure: {report:?}"));
        })
    }

    pub fn record(&self, report: DropFailureReport) {
        (self.callback)(report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn drop_report_is_sent_to_sink_without_losing_diagnostics() {
        let observed = Rc::new(RefCell::new(None));
        let observed_by_sink = observed.clone();
        let sink = CleanupSink::new(move |report| {
            *observed_by_sink.borrow_mut() = Some(report);
        });
        let diagnostic = CleanupFailureDiagnostic::new(
            CleanupOrigin::Root,
            silex_core::CloseError::from_panic(Box::new("drop failure")).into_diagnostic(),
        );

        sink.record(DropFailureReport::from_parts(vec![diagnostic], Vec::new()));
        let observed = observed.borrow();
        let report = observed.as_ref().expect("sink should observe the report");
        assert_eq!(report.cleanup_failures().len(), 1);
        assert_eq!(report.cleanup_failures()[0].origin(), CleanupOrigin::Root);
    }
}
