use crate::{AppHost, AppHostError, BootstrapError, HostState, UnmountOutcome};
use silex_core::{Runtime, SilexError, SilexErrorKind, SilexResult};
use silex_dom::{
    CleanupSink, MountContext,
    helpers::{self, detached::WindowListenerHandle},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use web_sys::Node;

/// Selects the browser events that can trigger an automatic page unmount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageLifecyclePolicy {
    /// Do not install a browser lifecycle listener.
    Manual,
    /// Unmount when the window dispatches `pagehide`.
    PageHide,
    /// Listen to `pagehide` and `visibilitychange`, but unmount only while the document is hidden.
    PageHideAndVisibilityChange,
}

/// Receives errors from an automatic lifecycle unmount.
pub type LifecycleReporter = Rc<dyn Fn(BootstrapError) + 'static>;

/// Owns an [`AppHost`] and optionally connects it to browser page lifecycle events.
pub struct PageController {
    lifecycle_listeners: Vec<WindowListenerHandle>,
    host: Option<Rc<RefCell<AppHost>>>,
}

impl PageController {
    /// Create a controller with an application-owned cleanup diagnostic sink.
    pub fn new(target: Node, cleanup_sink: CleanupSink) -> Self {
        Self {
            lifecycle_listeners: Vec::new(),
            host: Some(Rc::new(RefCell::new(AppHost::new(target, cleanup_sink)))),
        }
    }

    /// Mount one application through the underlying [`AppHost`].
    pub fn mount<F>(&mut self, runtime: Runtime, builder: F) -> Result<(), BootstrapError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        let mut host = self
            .host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow_mut()
            .map_err(|_| BootstrapError::Host(AppHostError::ReentrantOperation))?;
        host.mount(runtime, builder).map_err(BootstrapError::from)
    }

    /// Dispose the current application and mount a replacement through the underlying host.
    pub fn replace<F>(&mut self, runtime: Runtime, builder: F) -> Result<(), BootstrapError>
    where
        F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
    {
        let mut host = self
            .host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow_mut()
            .map_err(|_| BootstrapError::Host(AppHostError::ReentrantOperation))?;
        host.replace(runtime, builder).map_err(BootstrapError::from)
    }

    /// Explicitly dispose the current application.
    pub fn unmount(&mut self) -> Result<UnmountOutcome, BootstrapError> {
        let mut host = self
            .host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow_mut()
            .map_err(|_| BootstrapError::Host(AppHostError::ReentrantOperation))?;
        host.unmount().map_err(BootstrapError::from)
    }

    /// Install the requested page lifecycle policy, replacing any existing policy.
    pub fn install_page_lifecycle(
        &mut self,
        policy: PageLifecyclePolicy,
        reporter: LifecycleReporter,
    ) -> Result<(), BootstrapError> {
        self.remove_page_lifecycle();

        let events = match policy {
            PageLifecyclePolicy::Manual => return Ok(()),
            PageLifecyclePolicy::PageHide => &["pagehide"][..],
            PageLifecyclePolicy::PageHideAndVisibilityChange => {
                &["pagehide", "visibilitychange"][..]
            }
        };
        if helpers::try_window().is_none() {
            return Err(BootstrapError::Listener(SilexError::fatal(
                SilexErrorKind::Dom("Window not found".to_string()),
            )));
        }

        let document = match policy {
            PageLifecyclePolicy::PageHideAndVisibilityChange => {
                Some(helpers::try_document().ok_or_else(|| {
                    BootstrapError::Listener(SilexError::fatal(SilexErrorKind::Dom(
                        "Document not found".to_string(),
                    )))
                })?)
            }
            PageLifecyclePolicy::Manual | PageLifecyclePolicy::PageHide => None,
        };
        let only_when_hidden = document.is_some();
        let host = Rc::downgrade(self.host.as_ref().expect("page controller host is present"));
        let mut listeners = Vec::with_capacity(events.len());

        for &event_name in events {
            let host = host.clone();
            let reporter = reporter.clone();
            let document = document.clone();
            let listener =
                helpers::detached::try_window_event_listener_untyped(event_name, move |_event| {
                    if only_when_hidden
                        && document.as_ref().is_some_and(|document| !document.hidden())
                    {
                        return;
                    }
                    dispatch_lifecycle_unmount(&host, &reporter);
                })
                .map_err(|error| {
                    BootstrapError::Listener(SilexError::fatal(SilexErrorKind::from(error)))
                })?;
            listeners.push(listener);
        }

        self.lifecycle_listeners = listeners;
        Ok(())
    }

    /// Remove all page lifecycle listeners owned by this controller.
    pub fn remove_page_lifecycle(&mut self) {
        self.lifecycle_listeners.clear();
    }

    /// Return whether the underlying host currently owns an active application.
    pub fn is_active(&self) -> SilexResult<bool> {
        self.host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow()
            .map_err(|_| {
                SilexError::fatal(SilexErrorKind::Framework(
                    "page controller host is already borrowed".to_string(),
                ))
            })?
            .is_active()
    }

    /// Return the underlying host state.
    pub fn state(&self) -> HostState {
        self.host
            .as_ref()
            .expect("page controller host is present")
            .borrow()
            .state()
    }

    /// Return the caller-provided target node.
    pub fn target(&self) -> Node {
        self.host
            .as_ref()
            .expect("page controller host is present")
            .borrow()
            .target()
    }

    #[cfg(feature = "browser-bootstrap")]
    pub(crate) fn into_app_host(mut self) -> Result<AppHost, BootstrapError> {
        if !self.lifecycle_listeners.is_empty() {
            return Err(BootstrapError::Lifecycle(
                "page lifecycle listeners must be removed before host transfer".to_string(),
            ));
        }

        let host = self.host.take().expect("page controller host is present");
        match Rc::try_unwrap(host) {
            Ok(host) => Ok(host.into_inner()),
            Err(_) => Err(BootstrapError::Lifecycle(
                "page controller host is still shared".to_string(),
            )),
        }
    }
}

impl Drop for PageController {
    fn drop(&mut self) {
        self.remove_page_lifecycle();
    }
}

fn dispatch_lifecycle_unmount(host: &Weak<RefCell<AppHost>>, reporter: &LifecycleReporter) {
    let Some(host) = host.upgrade() else {
        return;
    };

    let result = match host.try_borrow_mut() {
        Ok(mut host) => host.unmount().map(|_| ()).map_err(BootstrapError::from),
        Err(_) => Err(BootstrapError::Host(AppHostError::ReentrantOperation)),
    };
    if let Err(error) = result {
        reporter(error);
    }
}
