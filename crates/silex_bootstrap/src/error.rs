use silex_core::SilexError;
use std::{error::Error, fmt};

use crate::app_host::AppHostError;

/// Errors reported by a bootstrap adapter around an [`AppHost`].
#[derive(Debug)]
pub enum BootstrapError {
    /// The underlying application host rejected the operation.
    Host(AppHostError),
    /// The requested browser target does not exist.
    TargetNotFound(String),
    /// An adapter lifecycle operation failed.
    Lifecycle(String),
    /// A browser listener could not be installed or removed.
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
