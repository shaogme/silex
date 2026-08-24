//! 应用 host 和 browser bootstrap 的错误所有权。

use silex_core::SilexError;
use silex_view::{DisposeError, MountError};
use std::{error::Error, fmt};

/// 应用 host 的生命周期状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Ready,
    Mounting,
    Active,
    Disposing,
    Poisoned,
}

/// 应用 host 的卸载结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnmountOutcome {
    Disposed,
    AlreadyUnmounted,
}

/// 应用 host 操作失败。
#[derive(Debug)]
pub enum AppHostError {
    AlreadyMounted,
    NotMounted,
    InvalidState { state: HostState },
    Mount(MountError),
    Dispose(DisposeError),
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
            Self::Mount(error) => Some(error),
            Self::Dispose(error) => Some(error),
            _ => None,
        }
    }
}

/// browser bootstrap 的统一错误。
#[derive(Debug)]
pub enum BootstrapError {
    Host(AppHostError),
    TargetNotFound(String),
    Lifecycle(String),
    Listener(SilexError),
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
            Self::Host(error) => Some(error),
            Self::Listener(error) => Some(error),
            Self::TargetNotFound(_) | Self::Lifecycle(_) => None,
        }
    }
}

impl From<AppHostError> for BootstrapError {
    fn from(error: AppHostError) -> Self {
        Self::Host(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_error_keeps_host_and_listener_sources() {
        let host = BootstrapError::from(AppHostError::AlreadyMounted);
        assert_eq!(
            host.to_string(),
            "bootstrap host operation failed: application host already has a mounted app"
        );
        assert!(host.source().is_some());

        let listener = BootstrapError::Listener(SilexError::fatal(
            silex_core::SilexErrorKind::Dom("window missing".to_string()),
        ));
        assert!(listener.source().is_some());
        assert_eq!(
            listener.to_string(),
            "bootstrap listener operation failed: Fatal: DOM Error: window missing"
        );
    }
}
