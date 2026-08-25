use silex_core::{
    CloseError, ClosePhase, CloseSource, CloseTransaction, EffectHandle, EffectPhase, ErrorHandler,
    ErrorHandlerAnchor, ErrorHandlerInput, HandlerLease, OwnerAccess, OwnerChild, ReactiveError,
    SilexError, SilexErrorKind, SilexResult, unwind_safe,
};
use silex_dom::{
    diagnostics::logging::console_error, lifecycle::node_ref::NodeRef, model::event::DomEvent,
    runtime::HostResource,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

mod state {
    use super::SilexResult;
    use silex_core::ReactiveError;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    pub(super) struct ActiveRegistrar<'scope> {
        inner: Rc<dyn Fn() -> bool + 'scope>,
    }

    impl<'scope> ActiveRegistrar<'scope> {
        pub(super) fn new<F>(is_active: F) -> Self
        where
            F: Fn() -> bool + 'scope,
        {
            Self {
                inner: Rc::new(is_active),
            }
        }

        pub(super) fn get(&self) -> bool {
            (self.inner)()
        }
    }

    #[doc(hidden)]
    pub struct SharedCell<T> {
        pub(super) inner: Rc<RefCell<T>>,
    }

    impl<T> Clone for SharedCell<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }

    impl<T> SharedCell<T> {
        pub fn new(value: T) -> Self {
            Self {
                inner: Rc::new(RefCell::new(value)),
            }
        }
        pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> R {
            callback(&self.inner.borrow())
        }
        pub fn with_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
            callback(&mut self.inner.borrow_mut())
        }
        pub fn replace(&self, value: T) -> T {
            self.inner.replace(value)
        }
        pub fn set(&self, value: T) {
            drop(self.replace(value));
        }
        pub fn take(&self) -> T
        where
            T: Default,
        {
            self.replace(T::default())
        }
    }

    pub struct MountState<'scope, T> {
        value: SharedCell<Option<T>>,
        active: ActiveRegistrar<'scope>,
    }

    impl<'scope, T: 'scope> MountState<'scope, T> {
        pub(super) fn new(value: T, active: ActiveRegistrar<'scope>) -> Self {
            Self {
                value: SharedCell::new(Some(value)),
                active,
            }
        }

        fn ensure_access(&self) -> SilexResult<()> {
            if self.active.get() {
                Ok(())
            } else {
                Err(silex_core::SilexError::fatal(ReactiveError::NoSuchNode))
            }
        }

        pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> SilexResult<R> {
            self.ensure_access()?;
            self.value.with(|value| {
                value
                    .as_ref()
                    .map(callback)
                    .ok_or(silex_core::SilexError::fatal(ReactiveError::NoSuchNode))
            })
        }

        pub fn update<R>(&self, callback: impl FnOnce(&mut T) -> R) -> SilexResult<R> {
            self.ensure_access()?;
            self.value.with_mut(|value| {
                value
                    .as_mut()
                    .map(callback)
                    .ok_or(silex_core::SilexError::fatal(ReactiveError::NoSuchNode))
            })
        }

        pub fn take(&self) -> SilexResult<T> {
            self.ensure_access()?;
            self.value
                .with_mut(Option::take)
                .ok_or(silex_core::SilexError::fatal(ReactiveError::NoSuchNode))
        }

        pub fn replace(&self, value: T) -> SilexResult<Option<T>> {
            self.ensure_access()?;
            Ok(self.value.with_mut(|current| current.replace(value)))
        }

        pub fn is_active(&self) -> bool {
            self.active.get()
        }

        #[doc(hidden)]
        pub fn take_for_cleanup(&self) -> Option<T> {
            self.value.with_mut(Option::take)
        }
    }

    impl<'scope, T: 'scope> Clone for MountState<'scope, T> {
        fn clone(&self) -> Self {
            Self {
                value: self.value.clone(),
                active: self.active.clone(),
            }
        }
    }
}

use state::ActiveRegistrar;
pub use state::{MountState, SharedCell};

pub type MountEffect<'scope> = Box<dyn FnMut() -> SilexResult<()> + 'scope>;
pub type MountCleanup<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;
pub type MountErrorHandler<'scope> = ErrorHandler<'scope>;
type MountErrorLease<'scope> = HandlerLease<'scope>;
pub(crate) type CleanupReporter = Rc<dyn Fn(CloseError)>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeOwnershipMode {
    Shared,
    BranchContent,
}

/// 共享一个 View 生命周期树的 runtime access 和 handler anchors。
pub struct MountOwnerContext<'scope> {
    access: OwnerAccess<'scope>,
    anchors: RefCell<Vec<ErrorHandlerAnchor<'scope>>>,
    cleanup_reporter: Option<CleanupReporter>,
    runtime_mode: RuntimeOwnershipMode,
}

impl<'scope> MountOwnerContext<'scope> {
    fn new(
        access: OwnerAccess<'scope>,
        reporter: Option<CleanupReporter>,
        mode: RuntimeOwnershipMode,
    ) -> Self {
        Self {
            access,
            anchors: RefCell::new(Vec::new()),
            cleanup_reporter: reporter,
            runtime_mode: mode,
        }
    }

    fn access(&self) -> OwnerAccess<'scope> {
        self.access
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

    fn close(&self) -> Result<(), CloseError> {
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
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| entry.handle.stop()))
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        let close_error = CloseError::from_panic(Box::new(format!(
                            "View effect stop failed: {error}"
                        )));
                        transaction.push_error(
                            ClosePhase::Effect,
                            CloseSource::Effect,
                            close_error.clone(),
                        );
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            entry
                                .close_handler
                                .handle(SilexError::fatal(SilexErrorKind::Close(close_error)))
                        }));
                        match result {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => transaction.push_error(
                                ClosePhase::Effect,
                                CloseSource::Handler,
                                CloseError::from_panic(Box::new(format!(
                                    "close handler failed: {error}"
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
                        Ok(Err(error)) => transaction.push_error(
                            ClosePhase::Cleanup,
                            CloseSource::Handler,
                            CloseError::from_panic(Box::new(format!(
                                "cleanup handler failed: {error}"
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
    ) -> SilexResult<silex_core::CompletionSender<DomEvent>>
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

/// View mount 时 owner 所需的最小能力。
pub trait MountOwner<'scope> {
    fn effect(
        &self,
        phase: EffectPhase,
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
        phase: EffectPhase,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        MountOwnerToken::effect(self, phase, callback, error_handler)
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
        phase: EffectPhase,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.token.effect(phase, callback, error_handler)
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
    use super::MountOwnerToken;
    use silex_core::{Runtime, SilexError, SilexErrorKind};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn cleanup_runs_in_owner_scope_and_dispatches_errors() {
        let observed = Rc::new(Cell::new(0));
        let observed_by_handler = observed.clone();
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|access| {
                let handler = access
                    .error_handler(move |_| observed_by_handler.set(observed_by_handler.get() + 1))
                    .expect("handler");
                let owner = MountOwnerToken::new(access);
                owner
                    .on_cleanup(
                        Box::new(|| {
                            Err(SilexError::recoverable(SilexErrorKind::Framework(
                                "cleanup".into(),
                            )))
                        }),
                        handler.view(),
                    )
                    .expect("cleanup");
                owner.close().expect("close should dispatch");
            })
            .expect("transient scope");
        assert_eq!(observed.get(), 1);
    }
}
