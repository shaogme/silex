use crate::{AppHost, AppHostError, BootstrapError, HostState, UnmountOutcome};
use silex_core::{DomError, Runtime, SilexError, SilexResult};
use silex_dom::{
    CleanupSink, DomContext, DomNode,
    browser::BrowserDom,
    event::{DomEventBridge, EventKind, EventSpec, WindowEventRequest},
    host::HostResource,
};
use silex_view::MountBuilderContext;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use web_sys::{Node, window};

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
pub type LifecycleReporter = Rc<dyn Fn(SilexError) + 'static>;

/// Owns an [`AppHost`] and optionally connects it to browser page lifecycle events.
pub struct PageController {
    dom: DomContext,
    lifecycle_listeners: Vec<HostResource<'static>>,
    host: Option<Rc<RefCell<AppHost>>>,
}

impl PageController {
    /// Construct a controller through the explicit browser adapter.
    pub fn from_web_sys(target: Node, cleanup_sink: CleanupSink) -> SilexResult<Self> {
        let document = window()
            .and_then(|window| window.document())
            .ok_or_else(|| SilexError::from(BootstrapError::TargetNotFound("document".into())))?;
        let browser = BrowserDom::new(document);
        let target = browser.from_web_sys_node(target)?;
        Ok(Self::new(browser.context(), target, cleanup_sink))
    }

    /// Create a controller with an application-owned cleanup diagnostic sink.
    pub fn new(dom: DomContext, target: DomNode, cleanup_sink: CleanupSink) -> Self {
        Self {
            lifecycle_listeners: Vec::new(),
            dom: dom.clone(),
            host: Some(Rc::new(RefCell::new(AppHost::new(
                dom,
                target,
                cleanup_sink,
            )))),
        }
    }

    /// Mount one application through the underlying [`AppHost`].
    pub fn mount<F>(&mut self, runtime: Runtime, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        let mut host = self
            .host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow_mut()
            .map_err(|_| SilexError::from(AppHostError::ReentrantOperation))?;
        host.mount(runtime, builder)
    }

    /// Dispose the current application and mount a replacement through the underlying host.
    pub fn replace<F>(&mut self, runtime: Runtime, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        let mut host = self
            .host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow_mut()
            .map_err(|_| SilexError::from(AppHostError::ReentrantOperation))?;
        host.replace(runtime, builder)
    }

    /// Explicitly dispose the current application.
    pub fn unmount(&mut self) -> SilexResult<UnmountOutcome> {
        let mut host = self
            .host
            .as_ref()
            .expect("page controller host is present")
            .try_borrow_mut()
            .map_err(|_| SilexError::from(AppHostError::ReentrantOperation))?;
        host.unmount()
    }

    /// Install the requested page lifecycle policy, replacing any existing policy.
    pub fn install_page_lifecycle(
        &mut self,
        policy: PageLifecyclePolicy,
        reporter: LifecycleReporter,
    ) -> SilexResult<()> {
        self.remove_page_lifecycle();

        let events = match policy {
            PageLifecyclePolicy::Manual => return Ok(()),
            PageLifecyclePolicy::PageHide => &["pagehide"][..],
            PageLifecyclePolicy::PageHideAndVisibilityChange => {
                &["pagehide", "visibilitychange"][..]
            }
        };
        let only_when_hidden = matches!(policy, PageLifecyclePolicy::PageHideAndVisibilityChange);
        let host = Rc::downgrade(self.host.as_ref().expect("page controller host is present"));
        let mut listeners = Vec::with_capacity(events.len());

        for &event_name in events {
            let host = host.clone();
            let reporter = reporter.clone();
            let dom = self.dom.clone();
            let bridge: Rc<dyn DomEventBridge> = Rc::new(move |_event| {
                if only_when_hidden && !dom.document_hidden().ok().flatten().unwrap_or(true) {
                    return Ok(());
                }
                dispatch_lifecycle_unmount(&host, &reporter);
                Ok(())
            });
            let listener = self
                .dom
                .listen_window(
                    WindowEventRequest::new(EventSpec::new(event_name, EventKind::Custom))
                        .with_bridge(bridge),
                )
                .map_err(listener_error)?;
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
            .map_err(|_| SilexError::from(AppHostError::ReentrantOperation))?
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
    pub fn target(&self) -> DomNode {
        self.host
            .as_ref()
            .expect("page controller host is present")
            .borrow()
            .target()
    }

    #[cfg(feature = "browser-bootstrap")]
    pub(crate) fn into_app_host(mut self) -> SilexResult<AppHost> {
        if !self.lifecycle_listeners.is_empty() {
            return Err(SilexError::from(BootstrapError::Lifecycle(
                "page lifecycle listeners must be removed before host transfer".to_string(),
            )));
        }

        let host = self.host.take().expect("page controller host is present");
        match Rc::try_unwrap(host) {
            Ok(host) => Ok(host.into_inner()),
            Err(_) => Err(SilexError::from(BootstrapError::Lifecycle(
                "page controller host is still shared".to_string(),
            ))),
        }
    }
}

impl Drop for PageController {
    fn drop(&mut self) {
        self.remove_page_lifecycle();
    }
}

fn listener_error(error: DomError) -> SilexError {
    SilexError::from(BootstrapError::Listener(Box::new(SilexError::from(error))))
}

fn dispatch_lifecycle_unmount(host: &Weak<RefCell<AppHost>>, reporter: &LifecycleReporter) {
    let Some(host) = host.upgrade() else {
        return;
    };

    let result = match host.try_borrow_mut() {
        Ok(mut host) => host.unmount().map(|_| ()),
        Err(_) => Err(SilexError::from(AppHostError::ReentrantOperation)),
    };
    if let Err(error) = result {
        reporter(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::SilexErrorKind;

    #[test]
    fn listener_error_keeps_the_dom_payload_nested_in_bootstrap() {
        let error = listener_error(DomError::Backend {
            operation: "listen_window",
            message: "window unavailable".to_string(),
        });

        assert!(matches!(
            error.kind(),
            SilexErrorKind::Bootstrap(bootstrap)
                if matches!(bootstrap.as_ref(), BootstrapError::Listener(listener)
                    if matches!(listener.kind(), SilexErrorKind::Dom(DomError::Backend {
                        operation: "listen_window",
                        message,
                    }) if message == "window unavailable"))
        ));
    }
}
