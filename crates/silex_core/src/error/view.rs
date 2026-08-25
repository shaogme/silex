use super::{CleanupReport, ErrorSeverity, SilexError, SilexErrorKind};
use std::{error::Error, fmt};

/// Whether a failed mount can be attempted again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountAvailability {
    Retryable,
    Poisoned,
}

/// A mount failure together with the result of its rollback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountError {
    inner: Box<MountErrorInner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MountErrorInner {
    primary: Box<SilexError>,
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
                primary: Box::new(primary),
                rollback,
                availability,
            }),
        }
    }

    pub fn poisoned(primary: SilexError) -> Self {
        Self {
            inner: Box::new(MountErrorInner {
                primary: Box::new(primary),
                rollback: CleanupReport::new(),
                availability: MountAvailability::Poisoned,
            }),
        }
    }

    pub fn poisoned_with_report(primary: SilexError, rollback: CleanupReport) -> Self {
        Self {
            inner: Box::new(MountErrorInner {
                primary: Box::new(primary),
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
        RollbackError::new(self.primary().clone(), self.rollback().clone())
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

    pub fn severity(&self) -> ErrorSeverity {
        if self.is_poisoned() {
            ErrorSeverity::Fatal
        } else {
            self.primary().severity()
        }
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport, MountAvailability) {
        let MountErrorInner {
            primary,
            rollback,
            availability,
        } = *self.inner;
        (*primary, rollback, availability)
    }
}

impl fmt::Display for MountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "application mount failed: {}", self.primary())
    }
}

impl Error for MountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary())
    }
}

/// A primary rollback error and the cleanup failures collected with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollbackError {
    primary: Box<SilexError>,
    report: CleanupReport,
}

impl RollbackError {
    pub fn new(primary: SilexError, report: CleanupReport) -> Self {
        Self {
            primary: Box::new(primary),
            report,
        }
    }

    pub fn primary(&self) -> &SilexError {
        &self.primary
    }

    pub fn report(&self) -> &CleanupReport {
        &self.report
    }

    pub fn severity(&self) -> ErrorSeverity {
        if self.report.is_clean() {
            self.primary().severity()
        } else {
            ErrorSeverity::Fatal
        }
    }

    pub fn into_parts(self) -> (SilexError, CleanupReport) {
        (*self.primary, self.report)
    }
}

impl fmt::Display for RollbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "view rollback failed after: {}", self.primary())
    }
}

impl Error for RollbackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary())
    }
}

/// A disposal failure containing the complete cleanup report.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    pub fn severity(&self) -> ErrorSeverity {
        ErrorSeverity::Fatal
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

impl Error for DisposeError {}

/// The structured error contract for View lifecycle operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    Mount(Box<MountError>),
    Rollback(Box<RollbackError>),
    Dispose(Box<DisposeError>),
    Invariant {
        operation: &'static str,
        message: String,
    },
}

impl ViewError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Mount(error) => error.severity(),
            Self::Rollback(error) => error.severity(),
            Self::Dispose(error) => error.severity(),
            Self::Invariant { .. } => ErrorSeverity::Fatal,
        }
    }

    pub fn mount_error(&self) -> Option<&MountError> {
        match self {
            Self::Mount(error) => Some(error),
            _ => None,
        }
    }

    pub fn rollback_error(&self) -> Option<&RollbackError> {
        match self {
            Self::Rollback(error) => Some(error),
            _ => None,
        }
    }

    pub fn dispose_error(&self) -> Option<&DisposeError> {
        match self {
            Self::Dispose(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for ViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mount(error) => error.fmt(formatter),
            Self::Rollback(error) => error.fmt(formatter),
            Self::Dispose(error) => error.fmt(formatter),
            Self::Invariant { operation, message } => write!(
                formatter,
                "view invariant failed during {operation}: {message}"
            ),
        }
    }
}

impl Error for ViewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Mount(error) => Some(error.as_ref()),
            Self::Rollback(error) => Some(error.as_ref()),
            Self::Dispose(_) | Self::Invariant { .. } => None,
        }
    }
}

impl From<MountError> for ViewError {
    fn from(error: MountError) -> Self {
        Self::Mount(Box::new(error))
    }
}

impl From<RollbackError> for ViewError {
    fn from(error: RollbackError) -> Self {
        Self::Rollback(Box::new(error))
    }
}

impl From<DisposeError> for ViewError {
    fn from(error: DisposeError) -> Self {
        Self::Dispose(Box::new(error))
    }
}

impl From<ViewError> for SilexErrorKind {
    fn from(error: ViewError) -> Self {
        Self::View(Box::new(error))
    }
}

impl From<MountError> for SilexErrorKind {
    fn from(error: MountError) -> Self {
        ViewError::from(error).into()
    }
}

impl From<RollbackError> for SilexErrorKind {
    fn from(error: RollbackError) -> Self {
        ViewError::from(error).into()
    }
}

impl From<DisposeError> for SilexErrorKind {
    fn from(error: DisposeError) -> Self {
        ViewError::from(error).into()
    }
}

impl From<ViewError> for SilexError {
    fn from(error: ViewError) -> Self {
        let severity = error.severity();
        SilexError::with_severity(error.into(), severity)
    }
}

impl From<MountError> for SilexError {
    fn from(error: MountError) -> Self {
        let severity = error.severity();
        SilexError::with_severity(error.into(), severity)
    }
}

impl From<RollbackError> for SilexError {
    fn from(error: RollbackError) -> Self {
        let severity = error.severity();
        SilexError::with_severity(error.into(), severity)
    }
}

impl From<DisposeError> for SilexError {
    fn from(error: DisposeError) -> Self {
        let severity = error.severity();
        SilexError::with_severity(error.into(), severity)
    }
}
