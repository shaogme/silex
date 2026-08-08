use silex_core::{Runtime, SilexResult};
use silex_dom::{CleanupSink, DisposeError, MountContext, MountError, MountedApp};
use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};
use web_sys::Node;

/// The lifecycle state of an application host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    /// No application is mounted and the host can accept a mount operation.
    Ready,
    /// A mount transaction is currently running.
    Mounting,
    /// A committed application is owned by this host.
    Active,
    /// The active application is currently being disposed.
    Disposing,
    /// Cleanup or rollback was not fully proven successful.
    Poisoned,
}

/// The result of an explicit unmount operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnmountOutcome {
    /// An active application was disposed.
    Disposed,
    /// The host was already empty.
    AlreadyUnmounted,
}

/// Errors returned by the application host state machine.
#[derive(Debug)]
pub enum AppHostError {
    /// `mount` was requested while another application was active.
    AlreadyMounted,
    /// An operation requiring an active application was requested while empty.
    NotMounted,
    /// The operation is not valid for the current host state.
    InvalidState { state: HostState },
    /// The underlying DOM mount transaction failed.
    Mount(MountError),
    /// The underlying mounted application could not be disposed cleanly.
    Dispose(DisposeError),
    /// An operation attempted to re-enter an in-progress host transition.
    ReentrantOperation,
    /// The host cannot prove that its framework-owned resources are clean.
    Poisoned,
}

impl AppHostError {
    /// Borrow the original mount error when this is a mount failure.
    pub fn mount_error(&self) -> Option<&MountError> {
        match self {
            Self::Mount(error) => Some(error),
            _ => None,
        }
    }

    /// Borrow the original dispose error when this is a disposal failure.
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
            Self::AlreadyMounted
            | Self::NotMounted
            | Self::InvalidState { .. }
            | Self::ReentrantOperation
            | Self::Poisoned => None,
        }
    }
}

/// Owns the single application mounted into a caller-provided DOM node.
pub struct AppHost {
    target: Node,
    active: Option<MountedApp>,
    cleanup_sink: CleanupSink,
    state: HostState,
}

impl AppHost {
    /// Create a host with an application-owned cleanup diagnostic sink.
    pub fn new(target: Node, cleanup_sink: CleanupSink) -> Self {
        Self {
            target,
            active: None,
            cleanup_sink,
            state: HostState::Ready,
        }
    }

    /// Create a host that reports Drop-only cleanup diagnostics to the console.
    pub fn with_console_sink(target: Node) -> Self {
        Self::new(target, CleanupSink::console())
    }

    /// Mount one application when this host is ready.
    pub fn mount<F>(&mut self, runtime: Runtime, builder: F) -> Result<(), AppHostError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        match self.state {
            HostState::Ready => {}
            HostState::Active => return Err(AppHostError::AlreadyMounted),
            HostState::Mounting | HostState::Disposing => {
                return Err(AppHostError::ReentrantOperation);
            }
            HostState::Poisoned => return Err(AppHostError::Poisoned),
        }

        self.state = HostState::Mounting;
        let result = catch_unwind(AssertUnwindSafe(|| {
            MountedApp::mount(
                runtime,
                self.target.clone(),
                self.cleanup_sink.clone(),
                builder,
            )
        }));

        match result {
            Ok(Ok(app)) => {
                self.active = Some(app);
                self.state = HostState::Active;
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = if error.rollback().is_clean() {
                    HostState::Ready
                } else {
                    HostState::Poisoned
                };
                Err(AppHostError::Mount(error))
            }
            Err(panic) => {
                self.active = None;
                self.state = HostState::Poisoned;
                resume_unwind(panic)
            }
        }
    }

    /// Dispose the current application and then mount a new one.
    pub fn replace<F>(&mut self, runtime: Runtime, builder: F) -> Result<(), AppHostError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        let active = match self.state {
            HostState::Ready => return Err(AppHostError::NotMounted),
            HostState::Active => self.active.take().ok_or(AppHostError::InvalidState {
                state: HostState::Active,
            })?,
            HostState::Mounting | HostState::Disposing => {
                return Err(AppHostError::ReentrantOperation);
            }
            HostState::Poisoned => return Err(AppHostError::Poisoned),
        };

        self.state = HostState::Disposing;
        let dispose_result = catch_unwind(AssertUnwindSafe(|| active.dispose()));
        match dispose_result {
            Ok(Ok(())) => {
                self.state = HostState::Ready;
                self.mount(runtime, builder)
            }
            Ok(Err(error)) => {
                self.state = HostState::Poisoned;
                Err(AppHostError::Dispose(error))
            }
            Err(panic) => {
                self.active = None;
                self.state = HostState::Poisoned;
                resume_unwind(panic)
            }
        }
    }

    /// Dispose the active application, if any.
    pub fn unmount(&mut self) -> Result<UnmountOutcome, AppHostError> {
        let active = match self.state {
            HostState::Ready => return Ok(UnmountOutcome::AlreadyUnmounted),
            HostState::Active => self.active.take().ok_or(AppHostError::InvalidState {
                state: HostState::Active,
            })?,
            HostState::Mounting | HostState::Disposing => {
                return Err(AppHostError::ReentrantOperation);
            }
            HostState::Poisoned => return Err(AppHostError::Poisoned),
        };

        self.state = HostState::Disposing;
        let dispose_result = catch_unwind(AssertUnwindSafe(|| active.dispose()));
        match dispose_result {
            Ok(Ok(())) => {
                self.state = HostState::Ready;
                Ok(UnmountOutcome::Disposed)
            }
            Ok(Err(error)) => {
                self.state = HostState::Poisoned;
                Err(AppHostError::Dispose(error))
            }
            Err(panic) => {
                self.active = None;
                self.state = HostState::Poisoned;
                resume_unwind(panic)
            }
        }
    }

    /// Whether this host currently owns an active mounted application.
    pub fn is_active(&self) -> bool {
        self.state == HostState::Active && self.active.as_ref().is_some_and(MountedApp::is_active)
    }

    /// Return the controller state without inspecting the DOM.
    pub fn state(&self) -> HostState {
        self.state
    }

    /// Return the caller-provided target node.
    pub fn target(&self) -> Node {
        self.target.clone()
    }
}
