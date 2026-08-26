use super::{
    context::{MountOwnerContext, RuntimeOwnershipMode},
    owner::{CleanupEntry, EffectEntry, LocalOwnerState},
    state::{ActiveRegistrar, MountState},
    types::{CleanupReporter, MountCleanup, MountEffect, MountErrorHandler, MountErrorLease},
};
use silex_core::{
    CloseError, CompletionSender, EffectPhase, ErrorHandlerInput, OwnerAccess, OwnerChild,
    ReactiveError, SilexError, SilexErrorKind, SilexResult, unwind_safe,
};
use silex_dom::{
    diagnostics::logging::console_error, lifecycle::node_ref::NodeRef, model::event::DomEvent,
    runtime::HostResource,
};
use std::rc::Rc;

#[derive(Clone)]
pub struct MountOwnerToken<'scope> {
    pub(crate) context: Rc<MountOwnerContext<'scope>>,
    pub(crate) state: Rc<LocalOwnerState<'scope>>,
}

impl<'scope> MountOwnerToken<'scope> {
    pub fn new(access: OwnerAccess<'scope>) -> Self {
        Self::from_context(Rc::new(MountOwnerContext::new(
            access,
            None,
            RuntimeOwnershipMode::Shared,
        )))
    }

    pub(crate) fn with_cleanup_reporter(
        access: OwnerAccess<'scope>,
        reporter: CleanupReporter,
    ) -> Self {
        Self::from_context(Rc::new(MountOwnerContext::new(
            access,
            Some(reporter),
            RuntimeOwnershipMode::Shared,
        )))
    }

    fn from_context(context: Rc<MountOwnerContext<'scope>>) -> Self {
        let state = Rc::new(LocalOwnerState::new(context.clone()));
        Self { context, state }
    }

    pub(crate) fn child(&self) -> Self {
        let state = Rc::new(LocalOwnerState::new(self.context.clone()));
        self.state.children.borrow_mut().push(state.clone());
        Self {
            context: self.context.clone(),
            state,
        }
    }

    pub(crate) fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, Self)> {
        self.ensure_active()?;
        let child = self.context.access().create_owned_child()?;
        let context = Rc::new(MountOwnerContext::new(
            child.access(),
            self.context.cleanup_reporter.clone(),
            RuntimeOwnershipMode::BranchContent,
        ));
        let state = Rc::new(LocalOwnerState::new(context.clone()));
        self.state.children.borrow_mut().push(state.clone());
        Ok((child, Self { context, state }))
    }

    pub fn close(&self) -> Result<(), CloseError> {
        self.state.close()
    }

    pub(crate) fn runtime_access(&self) -> OwnerAccess<'scope> {
        self.context.access()
    }

    pub fn effect<H>(
        &self,
        phase: EffectPhase,
        callback: MountEffect<'scope>,
        error_handler: H,
    ) -> SilexResult<()>
    where
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let handler = self.context.handler(error_handler.handler_ref())?;
        let close_handler = self.close_handler(handler)?;
        let handle = self
            .context
            .access()
            .effect_detached(phase, callback, handler)?;
        self.state.effects.borrow_mut().push(EffectEntry {
            handle,
            close_handler,
        });
        Ok(())
    }

    pub fn effect_with_previous<T, F, H>(
        &self,
        phase: EffectPhase,
        callback: F,
        error_handler: H,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let handler = self.context.handler(error_handler.handler_ref())?;
        let close_handler = self.close_handler(handler)?;
        let handle = self
            .context
            .access()
            .effect_with_previous(phase, callback, handler)?;
        self.state.effects.borrow_mut().push(EffectEntry {
            handle,
            close_handler,
        });
        Ok(())
    }

    pub fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<MountState<'scope, T>> {
        self.ensure_active()?;
        Ok(MountState::new(
            value,
            ActiveRegistrar::new({
                let state = self.state.clone();
                move || state.active.get()
            }),
        ))
    }

    pub fn node_ref(&self) -> NodeRef<'scope> {
        NodeRef::new()
    }

    pub fn on_cleanup<H>(&self, cleanup: MountCleanup<'scope>, error_handler: H) -> SilexResult<()>
    where
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let handler = self.context.handler(error_handler.handler_ref())?;
        let close_handler = self.close_handler(handler)?;
        self.state.cleanups.borrow_mut().push(CleanupEntry {
            cleanup,
            close_handler,
        });
        if !self.state.cleanup_registered.replace(true) {
            let state = self.state.clone();
            if let Err(error) = self.context.access().on_cleanup(
                move || {
                    state.close().map_err(|error| {
                        state.report(error.clone());
                        SilexError::fatal(SilexErrorKind::Close(error))
                    })
                },
                handler,
            ) {
                self.state.cleanup_registered.set(false);
                let _ = self.state.cleanups.borrow_mut().pop();
                return Err(error);
            }
        }
        Ok(())
    }

    /// 把低层 HostResource 交给 owner；取消先由 resource 自身关闭 gate。
    pub fn track_host_resource<H>(
        &self,
        resource: HostResource<'static>,
        error_handler: H,
    ) -> SilexResult<()>
    where
        H: ErrorHandlerInput<'scope>,
    {
        let result = self.on_cleanup(
            Box::new(move || resource.cancel().map_err(SilexError::from)),
            error_handler,
        );
        if result.is_err() { /* resource is dropped here and cancellation is idempotent */ }
        result
    }

    pub(crate) fn event_sender<F, H>(
        &self,
        mut callback: F,
        error_handler: H,
    ) -> SilexResult<CompletionSender<DomEvent>>
    where
        F: FnMut(DomEvent) -> SilexResult<()> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let handler = self.context.handler(error_handler.handler_ref())?;
        let lease = handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        self.context
            .access()
            .completion_sender_detached(unwind_safe(move |event| match callback(event) {
                Ok(()) => Ok(()),
                Err(error) => lease
                    .handle(error)
                    .map_err(|error| SilexError::fatal(ReactiveError::Handler(error))),
            }))
    }

    pub(crate) fn is_active(&self) -> SilexResult<bool> {
        if !self.state.active.get() {
            return Ok(false);
        }
        self.context.access().is_active()
    }

    #[doc(hidden)]
    pub fn with_runtime<R>(&self, callback: impl FnOnce() -> R) -> SilexResult<R> {
        self.context.access().with_runtime(callback)
    }

    pub(crate) fn report_close_error(&self, error: CloseError) {
        if self.context.cleanup_reporter.is_some() {
            self.state.report(error);
        } else {
            console_error(format!("Silex owner close failure: {error:?}"));
        }
    }

    fn close_handler(
        &self,
        handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountErrorLease<'scope>> {
        handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))
    }

    fn ensure_active(&self) -> SilexResult<()> {
        if self.is_active()? {
            Ok(())
        } else {
            Err(SilexError::fatal(ReactiveError::NoSuchNode))
        }
    }
}
