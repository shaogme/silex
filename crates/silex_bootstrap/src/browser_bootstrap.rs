use crate::{
    BootstrapError, HostState, JsAppHost, LifecycleReporter, PageController, PageLifecyclePolicy,
    UnmountOutcome,
};
use silex_core::{Runtime, SilexError, SilexResult};
use silex_dom::{CleanupSink, DomContext, DomNode, browser::BrowserDom};
use silex_view::MountBuilderContext;
use web_sys::{Element, Node, window};

/// A browser convenience adapter around [`PageController`].
///
/// This type resolves browser targets and delegates all application ownership and lifecycle
/// transitions to the existing controller. It does not register a global entry point.
pub struct BrowserBootstrap {
    controller: PageController,
    policy: PageLifecyclePolicy,
}

impl BrowserBootstrap {
    /// Create a browser bootstrap for a caller-owned DOM node.
    pub fn new(dom: DomContext, target: DomNode, cleanup_sink: CleanupSink) -> Self {
        Self {
            controller: PageController::new(dom, target, cleanup_sink),
            policy: PageLifecyclePolicy::Manual,
        }
    }

    /// Adapt a browser Node into the backend-neutral bootstrap handles.
    pub fn from_web_sys(target: Node, cleanup_sink: CleanupSink) -> SilexResult<Self> {
        let document = window()
            .and_then(|window| window.document())
            .ok_or_else(|| SilexError::from(BootstrapError::TargetNotFound("document".into())))?;
        let browser = BrowserDom::new(document);
        let node = browser.from_web_sys_node(target)?;
        Ok(Self::new(browser.context(), node, cleanup_sink))
    }

    /// Create a browser bootstrap for a caller-owned element.
    pub fn from_element(target: Element, cleanup_sink: CleanupSink) -> SilexResult<Self> {
        Self::from_web_sys(target.into(), cleanup_sink)
    }

    /// Resolve an element by id from the current document.
    pub fn from_id(id: &str, cleanup_sink: CleanupSink) -> SilexResult<Self> {
        let document = window()
            .and_then(|window| window.document())
            .ok_or_else(|| SilexError::from(BootstrapError::TargetNotFound("document".into())))?;
        let browser = BrowserDom::new(document.clone());
        let target = document
            .get_element_by_id(id)
            .map(Node::from)
            .ok_or_else(|| BootstrapError::TargetNotFound(id.to_string()))?;
        let target = browser.from_web_sys_node(target)?;
        Ok(Self::new(browser.context(), target, cleanup_sink))
    }

    /// Mount an application through the underlying controller.
    pub fn mount<F>(&mut self, runtime: Runtime, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        self.controller.mount(runtime, builder)
    }

    /// Dispose the active application and mount a replacement.
    pub fn replace<F>(&mut self, runtime: Runtime, builder: F) -> SilexResult<()>
    where
        F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
    {
        self.controller.replace(runtime, builder)
    }

    /// Explicitly dispose the active application.
    pub fn unmount(&mut self) -> SilexResult<UnmountOutcome> {
        self.controller.unmount()
    }

    /// Install or replace the explicit page lifecycle policy.
    pub fn install_page_lifecycle(
        &mut self,
        policy: PageLifecyclePolicy,
        reporter: LifecycleReporter,
    ) -> SilexResult<()> {
        self.policy = PageLifecyclePolicy::Manual;
        let result = self.controller.install_page_lifecycle(policy, reporter);
        if result.is_ok() {
            self.policy = policy;
        }
        result
    }

    /// Remove page lifecycle listeners and return to the manual ownership policy.
    pub fn remove_page_lifecycle(&mut self) {
        self.controller.remove_page_lifecycle();
        self.policy = PageLifecyclePolicy::Manual;
    }

    /// Return the current controller state.
    pub fn state(&self) -> HostState {
        self.controller.state()
    }

    /// Return whether an application is currently active.
    pub fn is_active(&self) -> SilexResult<bool> {
        self.controller.is_active()
    }

    /// Return the resolved target node.
    pub fn target(&self) -> DomNode {
        self.controller.target()
    }

    /// Transfer a listener-free manual controller to the JavaScript owner.
    ///
    /// The first adapter version does not transfer page listener ownership. Callers must use
    /// `Manual` policy and remove any previously installed lifecycle policy first.
    pub fn into_js_host(self) -> SilexResult<JsAppHost> {
        if self.policy != PageLifecyclePolicy::Manual {
            return Err(SilexError::from(BootstrapError::Lifecycle(
                "JavaScript host transfer requires Manual page lifecycle policy".to_string(),
            )));
        }

        self.controller
            .into_app_host()
            .map(JsAppHost::from_app_host)
    }
}
