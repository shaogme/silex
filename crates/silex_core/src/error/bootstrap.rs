use super::{DisposeError, MountError, SilexError};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Ready,
    Mounting,
    Active,
    Disposing,
    Poisoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnmountOutcome {
    Disposed,
    AlreadyUnmounted,
}

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
