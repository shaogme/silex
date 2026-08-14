use silex_core::{CleanupSink, Runtime, SilexResult};
use silex_dom::{MountContext, MountedApp};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use web_sys::Node;

pub use silex_core::{AppHostError, HostState, UnmountOutcome};

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
        let mut app = MountedApp::new(runtime, self.target.clone(), self.cleanup_sink.clone());
        let result = catch_unwind(AssertUnwindSafe(|| app.mount(builder)));

        match result {
            Ok(Ok(())) => {
                self.active = Some(app);
                self.state = HostState::Active;
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = if error.can_retry() {
                    HostState::Ready
                } else {
                    HostState::Poisoned
                };
                Err(AppHostError::Mount(error))
            }
            Err(panic) => {
                self.active = Some(app);
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
        let mut active = active;
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
        let mut active = active;
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
