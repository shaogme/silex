use super::{DisposeError, ErrorSeverity, MountError, SilexError, SilexErrorKind};
use std::{error::Error, fmt};

/// Application host lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Ready,
    Mounting,
    Active,
    Disposing,
    Poisoned,
}

/// Result of an unmount operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnmountOutcome {
    Disposed,
    AlreadyUnmounted,
}

/// Application host lifecycle failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppHostError {
    AlreadyMounted,
    NotMounted,
    InvalidState { state: HostState },
    Mount(Box<MountError>),
    Dispose(Box<DisposeError>),
    ReentrantOperation,
    Poisoned,
}

impl AppHostError {
    pub fn mount_error(&self) -> Option<&MountError> {
        match self {
            Self::Mount(error) => Some(error),
            _ => None,
        }
    }

    pub fn dispose_error(&self) -> Option<&DisposeError> {
        match self {
            Self::Dispose(error) => Some(error),
            _ => None,
        }
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::AlreadyMounted | Self::NotMounted | Self::ReentrantOperation => {
                ErrorSeverity::Recoverable
            }
            Self::Mount(error) => error.severity(),
            Self::Dispose(_) | Self::InvalidState { .. } | Self::Poisoned => ErrorSeverity::Fatal,
        }
    }
}

impl fmt::Display for AppHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyMounted => {
                formatter.write_str("application host already has a mounted app")
            }
            Self::NotMounted => formatter.write_str("application host has no mounted app"),
            Self::InvalidState { state } => {
                write!(formatter, "application host is in invalid state: {state:?}")
            }
            Self::Mount(error) => error.fmt(formatter),
            Self::Dispose(error) => error.fmt(formatter),
            Self::ReentrantOperation => {
                formatter.write_str("application host operation is reentrant")
            }
            Self::Poisoned => formatter.write_str("application host is poisoned"),
        }
    }
}

impl Error for AppHostError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Mount(error) => Some(error.as_ref()),
            Self::Dispose(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Unified browser bootstrap failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootstrapError {
    Host(Box<AppHostError>),
    TargetNotFound(String),
    Lifecycle(String),
    Listener(Box<SilexError>),
}

impl BootstrapError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Host(error) => error.severity(),
            Self::TargetNotFound(_) | Self::Lifecycle(_) => ErrorSeverity::Fatal,
            Self::Listener(error) => error.severity(),
        }
    }
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(error) => write!(formatter, "bootstrap host operation failed: {error}"),
            Self::TargetNotFound(target) => {
                write!(formatter, "bootstrap target not found: {target}")
            }
            Self::Lifecycle(message) => {
                write!(formatter, "bootstrap lifecycle operation failed: {message}")
            }
            Self::Listener(error) => {
                write!(formatter, "bootstrap listener operation failed: {error}")
            }
        }
    }
}

impl Error for BootstrapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Host(error) => Some(error.as_ref()),
            Self::Listener(error) => Some(error.as_ref()),
            Self::TargetNotFound(_) | Self::Lifecycle(_) => None,
        }
    }
}

impl From<AppHostError> for BootstrapError {
    fn from(error: AppHostError) -> Self {
        Self::Host(Box::new(error))
    }
}

impl From<BootstrapError> for SilexErrorKind {
    fn from(error: BootstrapError) -> Self {
        Self::Bootstrap(Box::new(error))
    }
}

impl From<AppHostError> for SilexErrorKind {
    fn from(error: AppHostError) -> Self {
        BootstrapError::from(error).into()
    }
}

impl From<BootstrapError> for SilexError {
    fn from(error: BootstrapError) -> Self {
        let severity = error.severity();
        SilexError::with_severity(error.into(), severity)
    }
}

impl From<AppHostError> for SilexError {
    fn from(error: AppHostError) -> Self {
        let severity = error.severity();
        SilexError::with_severity(error.into(), severity)
    }
}
