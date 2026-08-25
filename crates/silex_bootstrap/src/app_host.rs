use crate::error::{AppHostError, HostState, UnmountOutcome};
use silex_core::{
    BootstrapError, CloseError, DisposeError, MountError, Runtime, SilexError, SilexErrorKind,
    SilexResult, ViewError,
};
use silex_dom::{
    CleanupFailure as DomCleanupFailure, CleanupOrigin, CleanupReport, CleanupSink, DomContext,
    DomNode, browser::BrowserDom,
};
use silex_view::{MountBuilderContext, MountedApp};
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};
use web_sys::{Node, window};

/// Owns the single application mounted into a caller-provided DOM node.
pub struct AppHost {
    dom: DomContext,
    target: DomNode,
    active: Option<MountedApp>,
    cleanup_sink: CleanupSink,
    state: HostState,
}

impl AppHost {
    /// Construct an abstract host through the explicit browser adapter.
    pub fn from_web_sys(target: Node, cleanup_sink: CleanupSink) -> SilexResult<Self> {
        let document = window()
            .and_then(|window| window.document())
            .ok_or_else(|| SilexError::from(BootstrapError::TargetNotFound("document".into())))?;
        let browser = BrowserDom::new(document);
        let target = browser.from_web_sys_node(target)?;
        Ok(Self::new(browser.context(), target, cleanup_sink))
    }

    /// Create a host with an application-owned cleanup diagnostic sink.
    pub fn new(dom: DomContext, target: DomNode, cleanup_sink: CleanupSink) -> Self {
        Self {
            dom,
            target,
            active: None,
            cleanup_sink,
            state: HostState::Ready,
        }
    }

    /// Mount one application when this host is ready.
    pub fn mount<F>(&mut self, runtime: Runtime, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        match self.state {
            HostState::Ready => {}
            HostState::Active => return Err(SilexError::from(AppHostError::AlreadyMounted)),
            HostState::Mounting | HostState::Disposing => {
                return Err(SilexError::from(AppHostError::ReentrantOperation));
            }
            HostState::Poisoned => return Err(SilexError::from(AppHostError::Poisoned)),
        }

        self.state = HostState::Mounting;
        let mut app = MountedApp::new(
            runtime,
            self.dom.clone(),
            self.target.clone(),
            self.cleanup_sink.clone(),
        );
        let result = catch_unwind(AssertUnwindSafe(|| app.mount(builder)));

        match result {
            Ok(Ok(())) => {
                self.active = Some(app);
                self.state = HostState::Active;
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = state_after_mount_failure(&error);
                Err(error)
            }
            Err(panic) => {
                self.active = Some(app);
                self.state = HostState::Poisoned;
                Err(SilexError::from(MountError::poisoned(panic_error(
                    "application mount",
                    panic,
                ))))
            }
        }
    }

    /// Dispose the current application and then mount a new one.
    pub fn replace<F>(&mut self, runtime: Runtime, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        let active = match self.state {
            HostState::Ready => return Err(SilexError::from(AppHostError::NotMounted)),
            HostState::Active => self.active.take().ok_or_else(|| {
                SilexError::from(AppHostError::InvalidState {
                    state: HostState::Active,
                })
            })?,
            HostState::Mounting | HostState::Disposing => {
                return Err(SilexError::from(AppHostError::ReentrantOperation));
            }
            HostState::Poisoned => return Err(SilexError::from(AppHostError::Poisoned)),
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
                Err(error)
            }
            Err(panic) => {
                self.active = None;
                self.state = HostState::Poisoned;
                Err(SilexError::from(DisposeError::new(panic_report(panic))))
            }
        }
    }

    /// Dispose the active application, if any.
    pub fn unmount(&mut self) -> SilexResult<UnmountOutcome> {
        let active = match self.state {
            HostState::Ready => return Ok(UnmountOutcome::AlreadyUnmounted),
            HostState::Active => self.active.take().ok_or_else(|| {
                SilexError::from(AppHostError::InvalidState {
                    state: HostState::Active,
                })
            })?,
            HostState::Mounting | HostState::Disposing => {
                return Err(SilexError::from(AppHostError::ReentrantOperation));
            }
            HostState::Poisoned => return Err(SilexError::from(AppHostError::Poisoned)),
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
                Err(error)
            }
            Err(panic) => {
                self.active = None;
                self.state = HostState::Poisoned;
                Err(SilexError::from(DisposeError::new(panic_report(panic))))
            }
        }
    }

    /// Whether this host currently owns an active mounted application.
    pub fn is_active(&self) -> SilexResult<bool> {
        if self.state != HostState::Active {
            return Ok(false);
        }
        match self.active.as_ref() {
            Some(app) => app.is_active(),
            None => Err(SilexError::from(AppHostError::InvalidState {
                state: HostState::Active,
            })),
        }
    }

    /// Return the controller state without inspecting the DOM.
    pub fn state(&self) -> HostState {
        self.state
    }

    /// Return the caller-provided target node.
    pub fn target(&self) -> DomNode {
        self.target.clone()
    }
}

fn state_after_mount_failure(error: &SilexError) -> HostState {
    match error.kind() {
        SilexErrorKind::View(view_error) => match view_error.as_ref() {
            ViewError::Mount(mount_error) if mount_error.can_retry() => HostState::Ready,
            _ => HostState::Poisoned,
        },
        _ => HostState::Poisoned,
    }
}

fn panic_error(operation: &str, panic: Box<dyn Any + Send>) -> SilexError {
    let close_error = CloseError::from_panic(panic);
    SilexError::fatal(SilexErrorKind::Framework(format!(
        "{operation} panicked: {}",
        close_error.diagnostic().message()
    )))
}

fn panic_report(panic: Box<dyn Any + Send>) -> CleanupReport {
    CleanupReport::from_parts(
        vec![DomCleanupFailure::new(
            CleanupOrigin::Root,
            CloseError::from_panic(panic),
        )],
        Vec::new(),
    )
}
