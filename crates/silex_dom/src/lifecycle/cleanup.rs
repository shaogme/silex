//! DOM 清理失败和 drop 阶段诊断。

use crate::diagnostics::logging::console_error;
use silex_core::{CleanupDiagnostic, SilexError};
use std::rc::Rc;

pub use silex_core::error::dom::{CleanupFailure, CleanupOrigin, CleanupReport};

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
    use silex_core::CloseError;
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
            CloseError::from_panic(Box::new("drop failure")).into_diagnostic(),
        );

        sink.record(DropFailureReport::from_parts(vec![diagnostic], Vec::new()));
        let observed = observed.borrow();
        let report = observed.as_ref().expect("sink should observe the report");
        assert_eq!(report.cleanup_failures().len(), 1);
        assert_eq!(report.cleanup_failures()[0].origin(), CleanupOrigin::Root);
    }
}
