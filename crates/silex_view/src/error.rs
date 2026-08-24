//! View mount 生命周期错误。

use silex_core::{SilexError, SilexResult};
use silex_dom::error::CleanupReport;
use std::fmt;

/// mount 失败后应用句柄是否仍可重试。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountAvailability {
    Retryable,
    Poisoned,
}

/// 由 primary mount error 与 rollback 清理结果组成的错误。
#[derive(Debug)]
pub struct MountError {
    inner: Box<MountErrorInner>,
}

#[derive(Debug)]
struct MountErrorInner {
    primary: SilexError,
    rollback: CleanupReport,
    availability: MountAvailability,
}

impl MountError {
    pub fn new(primary: SilexError, rollback: CleanupReport) -> Self {
        let availability = if rollback.is_clean() {
            MountAvailability::Retryable
        } else {
            MountAvailability::Poisoned
        };
        Self {
            inner: Box::new(MountErrorInner {
                primary,
                rollback,
                availability,
            }),
        }
    }

    pub fn poisoned(primary: SilexError) -> Self {
        Self {
            inner: Box::new(MountErrorInner {
                primary,
                rollback: CleanupReport::new(),
                availability: MountAvailability::Poisoned,
            }),
        }
    }

    pub fn poisoned_with_report(primary: SilexError, rollback: CleanupReport) -> Self {
        Self {
            inner: Box::new(MountErrorInner {
                primary,
                rollback,
                availability: MountAvailability::Poisoned,
            }),
        }
    }

    pub fn primary(&self) -> &SilexError {
        &self.inner.primary
    }

    pub fn rollback(&self) -> &CleanupReport {
        &self.inner.rollback
    }

    pub fn rollback_error(&self) -> RollbackError {
        RollbackError::new(self.inner.primary.clone(), self.inner.rollback.clone())
    }

    pub fn availability(&self) -> MountAvailability {
        self.inner.availability
    }

    pub fn can_retry(&self) -> bool {
        self.availability() == MountAvailability::Retryable
    }

    pub fn is_poisoned(&self) -> bool {
        self.availability() == MountAvailability::Poisoned
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport, MountAvailability) {
        let MountErrorInner {
            primary,
            rollback,
            availability,
        } = *self.inner;
        (primary, rollback, availability)
    }
}

impl fmt::Display for MountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "application mount failed: {}", self.primary())
    }
}

impl std::error::Error for MountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.primary())
    }
}

/// rollback 的 primary error 和清理错误聚合。
#[derive(Debug)]
pub struct RollbackError {
    primary: SilexError,
    report: CleanupReport,
}

impl RollbackError {
    pub fn new(primary: SilexError, report: CleanupReport) -> Self {
        Self { primary, report }
    }

    pub fn primary(&self) -> &SilexError {
        &self.primary
    }

    pub fn report(&self) -> &CleanupReport {
        &self.report
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport) {
        (self.primary, self.report)
    }
}

impl fmt::Display for RollbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "view rollback failed after: {}", self.primary)
    }
}

impl std::error::Error for RollbackError {}

/// dispose 阶段的清理报告错误。
#[derive(Debug)]
pub struct DisposeError {
    report: CleanupReport,
}

impl DisposeError {
    pub fn new(report: CleanupReport) -> Self {
        Self { report }
    }

    pub fn report(&self) -> &CleanupReport {
        &self.report
    }

    pub fn into_parts(self) -> CleanupReport {
        self.report
    }
}

impl fmt::Display for DisposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("application disposal failed")
    }
}

impl std::error::Error for DisposeError {}

/// 将低层 DOM 错误保留为 View 的 fatal framework error。
pub(crate) fn dom_error(error: silex_dom::DomError) -> silex_core::SilexError {
    SilexError::fatal(silex_core::SilexErrorKind::Dom(error.to_string()))
}

pub(crate) fn unsupported(capability: &'static str) -> SilexResult<()> {
    Err(SilexError::fatal(silex_core::SilexErrorKind::Dom(format!(
        "unsupported capability: {capability}"
    ))))
}
