use super::host::{HostCallback, HostDestination, HostResource, HostResourceLease};
use super::state::{ActiveRegistrar, MountState};
use silex_core::{
    CloseError, ClosePhase, CloseSource, CloseTransaction, EffectHandle, ErrorHandler,
    ErrorHandlerAnchor, ErrorHandlerInput, HandlerLease, OwnerAccess, OwnerChild, ReactiveError,
    SilexError, SilexErrorKind, SilexResult, unwind_safe,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsValue;

/// An effect registered by one DOM owner token.
pub type MountEffect<'scope> = Box<dyn FnMut() -> SilexResult<()> + 'scope>;

/// A cleanup registered by one DOM owner token.
pub type MountCleanup<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;

pub type MountErrorHandler<'scope> = ErrorHandler<'scope>;
type MountErrorLease<'scope> = HandlerLease<'scope>;
pub(crate) type CleanupReporter = Rc<dyn Fn(CloseError)>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeOwnershipMode {
    Shared,
    BranchContent,
}

/// Shared owner context for one DOM lifecycle tree.
///
/// The context owns the stable handler anchors and the reactive access used by
/// every descendant token. A child token only creates a local lifecycle state;
/// it never tries to retain a handler again from a borrowed view.
pub struct MountOwnerContext<'scope> {
    access: OwnerAccess<'scope>,
    anchors: RefCell<Vec<ErrorHandlerAnchor<'scope>>>,
    cleanup_reporter: Option<CleanupReporter>,
    runtime_mode: RuntimeOwnershipMode,
}

impl<'scope> MountOwnerContext<'scope> {
    fn new(
        access: OwnerAccess<'scope>,
        cleanup_reporter: Option<CleanupReporter>,
        runtime_mode: RuntimeOwnershipMode,
    ) -> Self {
        Self {
            access,
            anchors: RefCell::new(Vec::new()),
            cleanup_reporter,
            runtime_mode,
        }
    }

    fn access(&self) -> OwnerAccess<'scope> {
        self.access
    }

    fn cleanup_reporter(&self) -> Option<CleanupReporter> {
        self.cleanup_reporter.clone()
    }

    fn owns_runtime_handles(&self) -> bool {
        self.runtime_mode == RuntimeOwnershipMode::Shared
    }

    fn handler(
        &self,
        requested: MountErrorHandler<'scope>,
    ) -> SilexResult<MountErrorHandler<'scope>> {
        if let Some(anchor) = self
            .anchors
            .borrow()
            .iter()
            .find(|anchor| anchor.view().is_same_handler(&requested))
        {
            return Ok(anchor.view());
        }

        let anchor = requested
            .anchor()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        let handler = anchor.view();
        self.anchors.borrow_mut().push(anchor);
        Ok(handler)
    }
}

struct EffectEntry<'scope> {
    handle: EffectHandle<'scope>,
    close_handler: MountErrorLease<'scope>,
}

struct CleanupEntry<'scope> {
    cleanup: MountCleanup<'scope>,
    close_handler: MountErrorLease<'scope>,
}

/// Local lifecycle state for one DOM subtree.
///
/// The reactive runtime owner is shared through `OwnerAccess`; this state is
/// the DOM-level boundary used to stop effects and dispose a subtree before
/// its parent owner is closed.
struct LocalOwnerState<'scope> {
    context: Rc<MountOwnerContext<'scope>>,
    active: Cell<bool>,
    closed: Cell<bool>,
    close_error: RefCell<Option<CloseError>>,
    effects: RefCell<Vec<EffectEntry<'scope>>>,
    cleanups: RefCell<Vec<CleanupEntry<'scope>>>,
    children: RefCell<Vec<Rc<LocalOwnerState<'scope>>>>,
    cleanup_registered: Cell<bool>,
    reported: Cell<bool>,
}

impl<'scope> LocalOwnerState<'scope> {
    fn new(context: Rc<MountOwnerContext<'scope>>) -> Self {
        Self {
            context,
            active: Cell::new(true),
            closed: Cell::new(false),
            close_error: RefCell::new(None),
            effects: RefCell::new(Vec::new()),
            cleanups: RefCell::new(Vec::new()),
            children: RefCell::new(Vec::new()),
            cleanup_registered: Cell::new(false),
            reported: Cell::new(false),
        }
    }

    fn close_dom_state(&self) -> Result<(), CloseError> {
        if self.closed.get() {
            return self.close_error.borrow().clone().map_or(Ok(()), Err);
        }

        self.active.set(false);
        let mut transaction = CloseTransaction::new();

        for child in self.children.borrow_mut().drain(..).rev() {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.close())) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    transaction.push_error(ClosePhase::Child, CloseSource::Child, error)
                }
                Err(panic) => transaction.push_error(
                    ClosePhase::Child,
                    CloseSource::Child,
                    CloseError::from_panic(panic),
                ),
            }
        }

        if self.context.owns_runtime_handles() {
            for entry in self.effects.borrow_mut().drain(..).rev() {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| entry.handle.stop()));
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        let close_error = CloseError::from_panic(Box::new(format!(
                            "DOM effect stop failed: {error}"
                        )));
                        transaction.push_error(
                            ClosePhase::Effect,
                            CloseSource::Effect,
                            close_error.clone(),
                        );
                        let dispatch =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                entry
                                    .close_handler
                                    .handle(SilexError::fatal(SilexErrorKind::Close(close_error)))
                            }));
                        match dispatch {
                            Ok(Ok(())) => {}
                            Ok(Err(handler_error)) => transaction.push_error(
                                ClosePhase::Effect,
                                CloseSource::Handler,
                                CloseError::from_panic(Box::new(format!(
                                    "DOM effect stop handler failed: {handler_error}"
                                ))),
                            ),
                            Err(panic) => transaction.push_error(
                                ClosePhase::Effect,
                                CloseSource::Handler,
                                CloseError::from_panic(panic),
                            ),
                        }
                    }
                    Err(panic) => transaction.push_error(
                        ClosePhase::Effect,
                        CloseSource::Effect,
                        CloseError::from_panic(panic),
                    ),
                }
            }
        } else {
            // Branch runtime owns these handles. The runtime may have already
            // recursively disposed them before this DOM state is reached, so
            // dropping the diagnostic records is the only safe DOM action.
            self.effects.borrow_mut().clear();
        }

        for entry in self.cleanups.borrow_mut().drain(..).rev() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(entry.cleanup));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    let dispatch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        entry.close_handler.handle(error)
                    }));
                    match dispatch {
                        Ok(Ok(())) => {}
                        Ok(Err(handler_error)) => transaction.push_error(
                            ClosePhase::Cleanup,
                            CloseSource::Handler,
                            CloseError::from_panic(Box::new(format!(
                                "DOM cleanup handler failed: {handler_error}"
                            ))),
                        ),
                        Err(panic) => transaction.push_error(
                            ClosePhase::Cleanup,
                            CloseSource::Handler,
                            CloseError::from_panic(panic),
                        ),
                    }
                }
                Err(panic) => transaction.push_error(
                    ClosePhase::Cleanup,
                    CloseSource::Cleanup,
                    CloseError::from_panic(panic),
                ),
            }
        }

        let result = transaction.finish();
        self.closed.set(true);
        self.close_error.replace(result.clone());
        result.map_or(Ok(()), Err)
    }

    fn close(&self) -> Result<(), CloseError> {
        self.close_dom_state()
    }

    fn report(&self, error: CloseError) {
        if !self.reported.replace(true)
            && let Some(reporter) = &self.context.cleanup_reporter
        {
            reporter(error);
        }
    }
}

#[derive(Clone)]
pub struct MountOwnerToken<'scope> {
    context: Rc<MountOwnerContext<'scope>>,
    state: Rc<LocalOwnerState<'scope>>,
}

impl<'scope> MountOwnerToken<'scope> {
    pub fn new(access: OwnerAccess<'scope>) -> Self {
        let context = Rc::new(MountOwnerContext::new(
            access,
            None,
            RuntimeOwnershipMode::Shared,
        ));
        Self::from_context(context)
    }

    pub(crate) fn with_cleanup_reporter(
        access: OwnerAccess<'scope>,
        cleanup_reporter: CleanupReporter,
    ) -> Self {
        let context = Rc::new(MountOwnerContext::new(
            access,
            Some(cleanup_reporter),
            RuntimeOwnershipMode::Shared,
        ));
        Self::from_context(context)
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
            self.context.cleanup_reporter(),
            RuntimeOwnershipMode::BranchContent,
        ));
        let state = Rc::new(LocalOwnerState::new(context.clone()));
        self.state.children.borrow_mut().push(state.clone());
        Ok((child, Self { context, state }))
    }

    pub(crate) fn close(&self) -> Result<(), CloseError> {
        self.state.close()
    }

    pub(crate) fn runtime_access(&self) -> OwnerAccess<'scope> {
        self.context.access()
    }

    pub fn effect<H>(&self, callback: MountEffect<'scope>, error_handler: H) -> SilexResult<()>
    where
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let error_handler = error_handler.handler_ref();
        let handler = self.context.handler(error_handler)?;
        let close_handler = self.close_handler(handler)?;
        let handle = self.context.access().effect_detached(callback, handler)?;
        self.state.effects.borrow_mut().push(EffectEntry {
            handle,
            close_handler,
        });
        Ok(())
    }

    pub fn effect_with_previous<T, F, H>(&self, callback: F, error_handler: H) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let error_handler = error_handler.handler_ref();
        let handler = self.context.handler(error_handler)?;
        let close_handler = self.close_handler(handler)?;
        let handle = self
            .context
            .access()
            .effect_with_previous(callback, handler)?;
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

    pub fn on_cleanup<H>(&self, cleanup: MountCleanup<'scope>, error_handler: H) -> SilexResult<()>
    where
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let error_handler = error_handler.handler_ref();
        let handler = self.context.handler(error_handler)?;
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

    pub(crate) fn host_callback<F, H>(
        &self,
        callback: F,
        error_handler: H,
    ) -> SilexResult<HostCallback>
    where
        F: FnMut(JsValue) -> SilexResult<()> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let error_handler = error_handler.handler_ref();
        Ok(HostCallback {
            destination: HostDestination::Sender(
                self.context
                    .access()
                    .completion_sender_detached(unwind_safe(callback))?,
            ),
            gate: Rc::new(Cell::new(true)),
            error_completion: self.error_completion(error_handler)?,
            state: Rc::new(Cell::new(super::host::CallbackState::Active)),
            close_failures: super::state::SharedCell::new(Vec::new()),
        })
    }

    pub(crate) fn host_callback_once<F, H>(
        &self,
        callback: F,
        error_handler: H,
    ) -> SilexResult<HostCallback>
    where
        F: FnMut(JsValue) -> SilexResult<()> + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let error_handler = error_handler.handler_ref();
        Ok(HostCallback {
            destination: HostDestination::Once(
                self.context
                    .access()
                    .completion_once_detached(unwind_safe(callback))?,
            ),
            gate: Rc::new(Cell::new(true)),
            error_completion: self.error_completion(error_handler)?,
            state: Rc::new(Cell::new(super::host::CallbackState::Active)),
            close_failures: super::state::SharedCell::new(Vec::new()),
        })
    }

    fn error_completion(
        &self,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<silex_core::CompletionSender<SilexError>> {
        let handler = self.context.handler(error_handler)?;
        let lease = handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        self.context
            .access()
            .completion_sender_detached(unwind_safe(move |error| {
                lease
                    .handle(error)
                    .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))
            }))
    }

    fn close_handler(
        &self,
        handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountErrorLease<'scope>> {
        handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))
    }

    pub(crate) fn is_active(&self) -> SilexResult<bool> {
        if !self.state.active.get() {
            return Ok(false);
        }
        self.context.access().is_active()
    }

    #[doc(hidden)]
    pub fn with_runtime<R>(&self, f: impl FnOnce() -> R) -> SilexResult<R> {
        self.context.access().with_runtime(f)
    }

    pub(crate) fn cleanup_reporter(&self) -> Option<CleanupReporter> {
        self.context.cleanup_reporter.clone()
    }

    pub(crate) fn report_close_error(&self, error: CloseError) {
        if self.cleanup_reporter().is_some() {
            self.state.report(error);
        } else {
            silex_core::log::console_error(format!("Silex owner close failure: {error:?}"));
        }
    }

    pub(crate) fn host_resource_for_callback<F, H>(
        &self,
        callback: &HostCallback,
        cancel: F,
        error_handler: H,
    ) -> SilexResult<HostResource<'scope>>
    where
        F: FnOnce() + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let callback_for_cancel = callback.clone();
        let resource = HostResource::with_gate(callback.gate.clone(), move || {
            let callback_error = callback_for_cancel.cancel().err();
            cancel();
            callback_error.map_or(Ok(()), Err)
        });
        self.register_host_resource(resource, error_handler)
    }

    pub(crate) fn host_resource_for_js_callback<F, H>(
        &self,
        callback: &HostCallback,
        resource: HostResource<'scope>,
        cancel: F,
        error_handler: H,
    ) -> SilexResult<HostResource<'scope>>
    where
        F: FnOnce() + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let callback_for_cancel = callback.clone();
        resource.install_cancel(move || {
            let callback_error = callback_for_cancel.cancel().err();
            cancel();
            callback_error.map_or(Ok(()), Err)
        });
        self.register_host_resource(resource, error_handler)
    }

    fn register_host_resource<H>(
        &self,
        resource: HostResource<'scope>,
        error_handler: H,
    ) -> SilexResult<HostResource<'scope>>
    where
        H: ErrorHandlerInput<'scope>,
    {
        self.ensure_active()?;
        let owner_resource: HostResourceLease<'scope> = resource.owner_lease();
        if let Err(error) = self.on_cleanup(
            Box::new(move || {
                owner_resource
                    .cancel_once()
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
            }),
            error_handler,
        ) {
            let _ = resource.cancel_once();
            return Err(error);
        }
        Ok(resource)
    }

    fn ensure_active(&self) -> SilexResult<()> {
        if self.is_active()? {
            Ok(())
        } else {
            Err(SilexError::fatal(ReactiveError::NoSuchNode))
        }
    }
}

/// Mount-time capability shared by all view implementations.
pub trait MountOwner<'scope> {
    fn effect(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;
    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;
    fn token(&self) -> MountOwnerToken<'scope>;
    fn child(&self) -> MountOwnerToken<'scope>;
    fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, MountOwnerToken<'scope>)>;
}

impl<'scope> MountOwner<'scope> for MountOwnerToken<'scope> {
    fn effect(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        MountOwnerToken::effect(self, callback, error_handler)
    }

    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        MountOwnerToken::on_cleanup(self, cleanup, error_handler)
    }

    fn token(&self) -> MountOwnerToken<'scope> {
        self.clone()
    }

    fn child(&self) -> MountOwnerToken<'scope> {
        MountOwnerToken::child(self)
    }

    fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, MountOwnerToken<'scope>)> {
        MountOwnerToken::branch_child(self)
    }
}

pub(crate) struct OwnerMount<'scope> {
    token: MountOwnerToken<'scope>,
}

impl<'scope> OwnerMount<'scope> {
    pub(crate) fn new(token: MountOwnerToken<'scope>) -> Self {
        Self { token }
    }
}

impl<'scope> MountOwner<'scope> for OwnerMount<'scope> {
    fn effect(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.token.effect(callback, error_handler)
    }

    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.token.on_cleanup(cleanup, error_handler)
    }

    fn token(&self) -> MountOwnerToken<'scope> {
        self.token.clone()
    }

    fn child(&self) -> MountOwnerToken<'scope> {
        self.token.child()
    }

    fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, MountOwnerToken<'scope>)> {
        self.token.branch_child()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::MountOwnerToken;
    use silex_core::{CloseSource, Runtime, SilexError, SilexErrorKind};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn cleanup_error_is_dispatched_through_close_safe_lease() {
        let reported = Rc::new(Cell::new(0));
        let reported_by_handler = reported.clone();
        let mut runtime = Runtime::new();

        let result = runtime
            .with_transient(|access| {
                let handler = access
                    .error_handler(move |_| {
                        reported_by_handler.set(reported_by_handler.get() + 1);
                    })
                    .expect("error handler should register");
                let owner = MountOwnerToken::new(access);
                owner.on_cleanup(
                    Box::new(|| {
                        Err(SilexError::recoverable(SilexErrorKind::Framework(
                            "cleanup failure".to_string(),
                        )))
                    }),
                    handler.view(),
                )
            })
            .expect("transient owner should close");

        result.expect("cleanup registration should succeed");
        assert_eq!(reported.get(), 1);
    }

    #[test]
    fn cleanup_lease_preserves_recoverable_and_fatal_errors() {
        let recoverable = Rc::new(Cell::new(0));
        let fatal = Rc::new(Cell::new(0));
        let recoverable_by_handler = recoverable.clone();
        let fatal_by_handler = fatal.clone();
        let mut runtime = Runtime::new();

        runtime
            .with_transient(|access| {
                let handler = access
                    .error_handler(move |error| match error {
                        SilexError::Recoverable(_) => {
                            recoverable_by_handler.set(recoverable_by_handler.get() + 1)
                        }
                        SilexError::Fatal(_) => fatal_by_handler.set(fatal_by_handler.get() + 1),
                    })
                    .expect("error handler should register");
                let owner = MountOwnerToken::new(access);
                owner
                    .on_cleanup(
                        Box::new(|| {
                            Err(SilexError::recoverable(SilexErrorKind::Framework(
                                "recoverable cleanup failure".to_string(),
                            )))
                        }),
                        handler.view(),
                    )
                    .expect("recoverable cleanup should register");
                owner
                    .on_cleanup(
                        Box::new(|| {
                            Err(SilexError::fatal(SilexErrorKind::Framework(
                                "fatal cleanup failure".to_string(),
                            )))
                        }),
                        handler.view(),
                    )
                    .expect("fatal cleanup should register");
                owner.close().expect("cleanup dispatch should succeed");
                Ok::<(), SilexError>(())
            })
            .expect("transient owner should close")
            .expect("test callback should succeed");

        assert_eq!(recoverable.get(), 1);
        assert_eq!(fatal.get(), 1);
    }

    #[test]
    fn cleanup_handler_panic_is_captured_as_a_close_failure() {
        let mut runtime = Runtime::new();
        let result = runtime.with_transient(|access| {
            let handler = access
                .error_handler(|_| panic!("cleanup handler panic"))
                .expect("error handler should register");
            let owner = MountOwnerToken::new(access);
            owner
                .on_cleanup(
                    Box::new(|| {
                        Err(SilexError::recoverable(SilexErrorKind::Framework(
                            "cleanup failure".to_string(),
                        )))
                    }),
                    handler.view(),
                )
                .expect("cleanup should register");

            let close = owner
                .close()
                .expect_err("handler panic should become a close error");
            assert!(
                close
                    .entries()
                    .iter()
                    .any(|entry| { entry.source() == CloseSource::Handler })
            );
            Ok::<(), SilexError>(())
        });

        assert!(result.is_err());
    }
}
