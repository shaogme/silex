use super::host::{CompletionRegistrar, HostCallback, HostDestination, HostResourceHandle};
use super::state::{ActiveRegistrar, MountState};
use silex_core::{
    CleanupError, ErrorReporter, OwnedScope, ReactiveError, Scope, SilexError, SilexResult,
    unwind_safe,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsValue;

/// Owner capabilities captured by a mounted view or attribute operation.
///
/// The token owns only registration functions. It never stores a borrowed
/// `MountOwner`, so an effect cannot outlive the adapter stack frame used by
/// the original mount call.
pub type MountEffect<'scope> = Box<dyn FnMut() -> SilexResult<()> + 'scope>;
pub type MountCleanup<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;
pub type MountErrorHandler<'scope> = ErrorReporter<'scope>;
pub(crate) type CleanupReporter = Rc<dyn Fn(CleanupError)>;

#[derive(Clone)]
struct EffectRegistrar<'scope> {
    inner: Rc<dyn EffectRegister<'scope> + 'scope>,
}

trait EffectRegister<'scope> {
    fn register(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;
}

impl<'scope, F> EffectRegister<'scope> for F
where
    F: Fn(MountEffect<'scope>, MountErrorHandler<'scope>) -> SilexResult<()> + 'scope,
{
    fn register(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self(callback, error_handler)
    }
}

impl<'scope> EffectRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(MountEffect<'scope>, MountErrorHandler<'scope>) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.inner.register(callback, error_handler)
    }
}

#[derive(Clone)]
struct CleanupRegistrar<'scope> {
    inner: Rc<dyn CleanupRegister<'scope> + 'scope>,
}

trait CleanupRegister<'scope> {
    fn register(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;
}

impl<'scope, F> CleanupRegister<'scope> for F
where
    F: Fn(MountCleanup<'scope>, MountErrorHandler<'scope>) -> SilexResult<()> + 'scope,
{
    fn register(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self(cleanup, error_handler)
    }
}

impl<'scope> CleanupRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(MountCleanup<'scope>, MountErrorHandler<'scope>) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.inner.register(cleanup, error_handler)
    }
}

#[derive(Clone)]
struct OwnedScopeRegistrar<'scope> {
    inner: Rc<dyn OwnedScopeRegister<'scope> + 'scope>,
}

trait OwnedScopeRegister<'scope> {
    fn create(&self) -> SilexResult<OwnedScope<'scope>>;
}

impl<'scope, F> OwnedScopeRegister<'scope> for F
where
    F: Fn() -> SilexResult<OwnedScope<'scope>> + 'scope,
{
    fn create(&self) -> SilexResult<OwnedScope<'scope>> {
        self()
    }
}

impl<'scope> OwnedScopeRegistrar<'scope> {
    fn new<F>(create: F) -> Self
    where
        F: Fn() -> SilexResult<OwnedScope<'scope>> + 'scope,
    {
        Self {
            inner: Rc::new(create),
        }
    }

    fn call(&self) -> SilexResult<OwnedScope<'scope>> {
        self.inner.create()
    }
}

#[derive(Clone)]
enum PreviousEffectOwner<'scope> {
    Scoped(Scope<'scope>),
    Owned(Rc<OwnedScope<'scope>>),
}

impl<'scope> PreviousEffectOwner<'scope> {
    fn register<T, F>(
        &self,
        callback: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        match self {
            Self::Scoped(scope) => scope
                .effect_with_previous(callback, error_handler)
                .map(|_| ()),
            Self::Owned(scope) => scope
                .effect_with_previous(callback, error_handler)
                .map(|_| ()),
        }
    }
}

#[derive(Clone)]
pub struct MountOwnerToken<'scope> {
    effect: EffectRegistrar<'scope>,
    previous_effect: PreviousEffectOwner<'scope>,
    cleanup: CleanupRegistrar<'scope>,
    owned_scope: OwnedScopeRegistrar<'scope>,
    completion: CompletionRegistrar<'scope>,
    active: ActiveRegistrar<'scope>,
    state_scope: Option<Scope<'scope>>,
    cleanup_reporter: Option<CleanupReporter>,
}

struct ViewOwnerTokenParts<'scope> {
    effect: EffectRegistrar<'scope>,
    previous_effect: PreviousEffectOwner<'scope>,
    cleanup: CleanupRegistrar<'scope>,
    owned_scope: OwnedScopeRegistrar<'scope>,
    completion: CompletionRegistrar<'scope>,
    active: ActiveRegistrar<'scope>,
    state_scope: Option<Scope<'scope>>,
    cleanup_reporter: Option<CleanupReporter>,
}

impl<'scope> MountOwnerToken<'scope> {
    fn new(parts: ViewOwnerTokenParts<'scope>) -> Self {
        Self {
            effect: parts.effect,
            previous_effect: parts.previous_effect,
            cleanup: parts.cleanup,
            owned_scope: parts.owned_scope,
            completion: parts.completion,
            active: parts.active,
            state_scope: parts.state_scope,
            cleanup_reporter: parts.cleanup_reporter,
        }
    }

    pub fn effect(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.effect.call(callback, error_handler)
    }

    pub fn effect_with_previous<T, F>(
        &self,
        callback: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.previous_effect.register(callback, error_handler)
    }

    pub fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<MountState<'scope, T>> {
        if !self.is_active() {
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        Ok(match self.state_scope {
            Some(scope) => MountState::new_stored(scope.stored(Some(value))?, self.active.clone()),
            None => MountState::new(value, self.active.clone()),
        })
    }

    pub fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.cleanup.call(cleanup, error_handler)
    }

    pub fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        self.owned_scope.call()
    }

    pub(crate) fn host_callback<F>(
        &self,
        callback: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<HostCallback>
    where
        F: FnMut(JsValue) -> SilexResult<()> + 'scope,
    {
        Ok(HostCallback {
            destination: HostDestination::Sender(self.completion.call_sender(Box::new(callback))?),
            gate: Rc::new(Cell::new(true)),
            error_completion: self.completion.call_error_sender(error_handler)?,
        })
    }

    pub(crate) fn host_callback_once<F>(
        &self,
        callback: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<HostCallback>
    where
        F: FnMut(JsValue) -> SilexResult<()> + 'scope,
    {
        Ok(HostCallback {
            destination: HostDestination::Once(self.completion.call_once(Box::new(callback))?),
            gate: Rc::new(Cell::new(true)),
            error_completion: self.completion.call_error_sender(error_handler)?,
        })
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.get()
    }

    pub(crate) fn cleanup_reporter(&self) -> Option<CleanupReporter> {
        self.cleanup_reporter.clone()
    }

    pub(crate) fn host_resource_for_callback<F>(
        &self,
        callback: &HostCallback,
        cancel: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<HostResourceHandle<'scope>>
    where
        F: FnOnce() + 'scope,
    {
        let callback_for_cancel = callback.clone();
        let resource = HostResourceHandle::with_gate(callback.gate.clone(), move || {
            callback_for_cancel.cancel();
            cancel();
        });
        self.register_host_resource(resource, error_handler)
    }

    pub(crate) fn host_resource_for_js_callback<F>(
        &self,
        callback: &HostCallback,
        resource: HostResourceHandle<'scope>,
        cancel: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<HostResourceHandle<'scope>>
    where
        F: FnOnce() + 'scope,
    {
        let callback_for_cancel = callback.clone();
        resource.install_cancel(move || {
            callback_for_cancel.cancel();
            cancel();
        });
        self.register_host_resource(resource, error_handler)
    }

    fn register_host_resource(
        &self,
        resource: HostResourceHandle<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<HostResourceHandle<'scope>> {
        if !self.is_active() {
            resource.cancel_once();
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        let owner_resource = resource.clone();
        if let Err(error) = self.on_cleanup(
            Box::new(move || {
                owner_resource.cancel_once();
                Ok(())
            }),
            error_handler,
        ) {
            resource.cancel_once();
            return Err(error);
        }
        Ok(resource)
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
    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>>;
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

    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        MountOwnerToken::owned_scope(self)
    }
}

/// Adapter for a lexical child scope.
#[derive(Clone)]
pub struct ScopedMountOwner<'scope> {
    scope: Scope<'scope>,
    cleanup_reporter: Option<CleanupReporter>,
}

impl<'scope> ScopedMountOwner<'scope> {
    pub fn new(scope: Scope<'scope>) -> Self {
        Self {
            scope,
            cleanup_reporter: None,
        }
    }

    pub(crate) fn with_cleanup_reporter(
        scope: Scope<'scope>,
        cleanup_reporter: CleanupReporter,
    ) -> Self {
        Self {
            scope,
            cleanup_reporter: Some(cleanup_reporter),
        }
    }

    pub fn effect_with_previous<T, F>(
        &self,
        callback: F,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.scope
            .effect_with_previous(callback, error_handler)
            .map(|_| ())
    }

    pub fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<MountState<'scope, T>> {
        self.token().owner_state(value)
    }
}

impl<'scope> MountOwner<'scope> for ScopedMountOwner<'scope> {
    fn effect(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope.effect(callback, error_handler).map(|_| ())
    }

    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope.on_cleanup(cleanup, error_handler)
    }

    fn token(&self) -> MountOwnerToken<'scope> {
        let scope_for_effect = self.scope;
        let scope_for_previous = self.scope;
        let scope_for_cleanup = self.scope;
        let scope_for_owned = self.scope;
        let scope_for_sender = self.scope;
        let scope_for_once = self.scope;
        let scope_for_error_sender = self.scope;
        let scope_for_active = self.scope;
        let cleanup_reporter = self.cleanup_reporter.clone();
        MountOwnerToken::new(ViewOwnerTokenParts {
            effect: EffectRegistrar::new(move |callback, error_handler| {
                scope_for_effect.effect(callback, error_handler).map(|_| ())
            }),
            previous_effect: PreviousEffectOwner::Scoped(scope_for_previous),
            cleanup: CleanupRegistrar::new(move |cleanup, error_handler| {
                scope_for_cleanup.on_cleanup(cleanup, error_handler)
            }),
            owned_scope: OwnedScopeRegistrar::new(move || scope_for_owned.owned_scope()),
            completion: CompletionRegistrar::new(
                move |callback| scope_for_sender.completion_sender(unwind_safe(callback)),
                move |callback| scope_for_once.completion_once(unwind_safe(callback)),
                move |error_handler| {
                    scope_for_error_sender.completion_sender(unwind_safe(move |error| {
                        error_handler
                            .handle(error)
                            .map_err(|error| SilexError::fatal(error.reason()))
                    }))
                },
            ),
            active: ActiveRegistrar::new(move || scope_for_active.is_active()),
            state_scope: Some(self.scope),
            cleanup_reporter,
        })
    }

    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        self.scope.owned_scope()
    }
}

pub(crate) struct OwnedMountOwner<'scope> {
    scope: Rc<OwnedScope<'scope>>,
    cleanup_reporter: Option<CleanupReporter>,
}

impl<'scope> OwnedMountOwner<'scope> {
    pub(crate) fn new(scope: Rc<OwnedScope<'scope>>) -> Self {
        Self {
            scope,
            cleanup_reporter: None,
        }
    }

    pub(crate) fn with_cleanup_reporter(
        scope: Rc<OwnedScope<'scope>>,
        cleanup_reporter: CleanupReporter,
    ) -> Self {
        Self {
            scope,
            cleanup_reporter: Some(cleanup_reporter),
        }
    }

    pub(crate) fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<MountState<'scope, T>> {
        self.token().owner_state(value)
    }
}

impl<'scope> MountOwner<'scope> for OwnedMountOwner<'scope> {
    fn effect(
        &self,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope.effect(callback, error_handler).map(|_| ())
    }

    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope.on_cleanup(cleanup, error_handler)
    }

    fn token(&self) -> MountOwnerToken<'scope> {
        let scope_for_effect = self.scope.clone();
        let scope_for_previous = self.scope.clone();
        let scope_for_cleanup = self.scope.clone();
        let scope_for_owned = self.scope.clone();
        let scope_for_sender = self.scope.clone();
        let scope_for_once = self.scope.clone();
        let scope_for_error_sender = self.scope.clone();
        let scope_for_active = self.scope.clone();
        let cleanup_reporter = self.cleanup_reporter.clone();
        MountOwnerToken::new(ViewOwnerTokenParts {
            effect: EffectRegistrar::new(move |callback, error_handler| {
                scope_for_effect.effect(callback, error_handler).map(|_| ())
            }),
            previous_effect: PreviousEffectOwner::Owned(scope_for_previous),
            cleanup: CleanupRegistrar::new(move |cleanup, error_handler| {
                scope_for_cleanup.on_cleanup(cleanup, error_handler)
            }),
            owned_scope: OwnedScopeRegistrar::new(move || scope_for_owned.child()),
            completion: CompletionRegistrar::new(
                move |callback| scope_for_sender.completion_sender(unwind_safe(callback)),
                move |callback| scope_for_once.completion_once(unwind_safe(callback)),
                move |error_handler| {
                    scope_for_error_sender.completion_sender(unwind_safe(move |error| {
                        error_handler
                            .handle(error)
                            .map_err(|error| SilexError::fatal(error.reason()))
                    }))
                },
            ),
            active: ActiveRegistrar::new(move || scope_for_active.is_active()),
            state_scope: None,
            cleanup_reporter,
        })
    }

    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        self.scope.child()
    }
}
#[cfg(test)]
mod tests {
    use super::{HostResourceHandle, MountOwner, ScopedMountOwner};
    use silex_core::{Runtime, SilexError, SilexErrorKind};
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    use wasm_bindgen::JsValue;

    #[test]
    fn host_callback_is_gated_after_root_dispose() {
        let seen = Rc::new(Cell::new(0));
        let seen_in_callback = seen.clone();
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        let bridge = {
            let scope = root.scope();
            let owner = ScopedMountOwner::new(scope);
            let token = owner.token();
            let handler = scope
                .error_handler(|_| {})
                .expect("error handler should register");
            let bridge = token
                .host_callback(
                    move |_| {
                        seen_in_callback.set(seen_in_callback.get() + 1);
                        Ok(())
                    },
                    handler,
                )
                .expect("host callback should register");
            assert!(bridge.dispatch(JsValue::UNDEFINED));
            bridge
        };

        assert_eq!(seen.get(), 1);
        root.dispose().expect("root cleanup should succeed");
        assert!(!bridge.dispatch(JsValue::UNDEFINED));
        assert_eq!(seen.get(), 1);
    }

    #[test]
    fn host_callback_reports_completion_errors_after_callback_returns() {
        let handled = Rc::new(Cell::new(0));
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        let bridge = {
            let scope = root.scope();
            let (_, set_signal) = scope.signal(0).expect("signal should initialize");
            let handled_in_handler = handled.clone();
            let handler = scope
                .error_handler(move |error| {
                    assert!(matches!(
                        error,
                        SilexError::Fatal(SilexErrorKind::Framework(message)) if message == "host"
                    ));
                    handled_in_handler.set(handled_in_handler.get() + 1);
                    set_signal.set(1).expect("signal should be writable");
                })
                .expect("error handler should register");
            let owner = ScopedMountOwner::new(scope);
            owner
                .token()
                .host_callback(
                    |_| {
                        Err(SilexError::fatal(SilexErrorKind::Framework(String::from(
                            "host",
                        ))))
                    },
                    handler,
                )
                .expect("host callback should register")
        };

        assert!(bridge.dispatch(JsValue::UNDEFINED));
        assert!(bridge.dispatch(JsValue::UNDEFINED));
        assert_eq!(handled.get(), 2);
    }

    #[test]
    fn host_callback_handler_panic_closes_the_destination() {
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        let bridge = {
            let scope = root.scope();
            let handler = scope
                .error_handler(|_| panic!("host error handler panic"))
                .expect("error handler should register");
            let owner = ScopedMountOwner::new(scope);
            owner
                .token()
                .host_callback(
                    |_| {
                        Err(SilexError::fatal(SilexErrorKind::Framework(String::from(
                            "host",
                        ))))
                    },
                    handler,
                )
                .expect("host callback should register")
        };

        let result = catch_unwind(AssertUnwindSafe(|| bridge.dispatch(JsValue::UNDEFINED)));
        assert!(result.is_err());
        assert!(!bridge.dispatch(JsValue::UNDEFINED));
        root.dispose().expect("root cleanup should succeed");
    }

    #[test]
    fn host_resource_cancellation_is_idempotent() {
        let cancelled = Rc::new(Cell::new(0));
        let cancelled_in_cleanup = cancelled.clone();
        let handle = HostResourceHandle::with_gate(Rc::new(Cell::new(true)), move || {
            cancelled_in_cleanup.set(cancelled_in_cleanup.get() + 1);
        });
        let clone = handle.clone();

        handle.cancel();
        clone.cancel();
        drop(clone);
        drop(handle);

        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn owner_keeps_resource_alive_when_returned_handle_is_dropped() {
        let cancelled = Rc::new(Cell::new(0));
        let cancelled_in_cleanup = cancelled.clone();
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        {
            let scope = root.scope();
            let owner = ScopedMountOwner::new(scope);
            let token = owner.token();
            let handler = scope
                .error_handler(|_| {})
                .expect("error handler should register");
            let callback = token
                .host_callback(|_| Ok(()), handler)
                .expect("host callback should register");
            let handle = token
                .host_resource_for_callback(
                    &callback,
                    move || {
                        cancelled_in_cleanup.set(cancelled_in_cleanup.get() + 1);
                    },
                    handler,
                )
                .expect("host resource should register");
            drop(handle);
        }
        assert_eq!(cancelled.get(), 0);
        root.dispose().expect("root cleanup should succeed");
        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn owner_effect_tracks_ordinary_reads() {
        let mut runtime = Runtime::new();
        let runs = Rc::new(Cell::new(0));

        runtime
            .child(|scope| {
                let (source, set_source) = scope.signal(1_i32).expect("signal should initialize");
                let handler = scope
                    .error_handler(|_| {})
                    .expect("error handler should register");
                let owner = ScopedMountOwner::new(scope);
                let runs_for_effect = runs.clone();
                owner
                    .effect(
                        Box::new(move || {
                            source.get()?;
                            runs_for_effect.set(runs_for_effect.get() + 1);
                            Ok(())
                        }),
                        handler,
                    )
                    .expect("owner effect should initialize");
                assert_eq!(runs.get(), 1);
                set_source.set(2).expect("source should update");
                assert_eq!(runs.get(), 2);
            })
            .expect("child scope should initialize");
    }

    #[test]
    fn explicit_handlers_route_errors_locally() {
        let outer_errors = Rc::new(RefCell::new(Vec::<String>::new()));
        let inner_errors = Rc::new(RefCell::new(Vec::<String>::new()));
        let outer_errors_for_reporter = outer_errors.clone();
        let inner_errors_for_reporter = inner_errors.clone();
        let mut runtime = Runtime::new();

        runtime
            .child(|scope| {
                let outer_handler = scope
                    .error_handler(move |error| {
                        outer_errors_for_reporter
                            .borrow_mut()
                            .push(error.to_string());
                    })
                    .expect("error handler should register");
                let inner_handler = scope
                    .error_handler(move |error| {
                        inner_errors_for_reporter
                            .borrow_mut()
                            .push(error.to_string());
                    })
                    .expect("error handler should register");
                outer_handler
                    .handle(SilexError::fatal(SilexErrorKind::Framework(
                        "outer".to_string(),
                    )))
                    .expect("outer handler should be active");
                inner_handler
                    .handle(SilexError::fatal(SilexErrorKind::Framework(
                        "inner".to_string(),
                    )))
                    .expect("inner handler should be active");
                outer_handler
                    .handle(SilexError::fatal(SilexErrorKind::Framework(
                        "outer-again".to_string(),
                    )))
                    .expect("outer handler should be active");
            })
            .expect("child scope should initialize");

        assert_eq!(
            outer_errors.borrow().as_slice(),
            [
                "Fatal: Framework Error: outer",
                "Fatal: Framework Error: outer-again"
            ]
        );
        assert_eq!(
            inner_errors.borrow().as_slice(),
            ["Fatal: Framework Error: inner"]
        );
    }

    #[test]
    fn owner_state_rejects_late_access_but_cleanup_can_take_value() {
        let late_access_rejected = Rc::new(Cell::new(false));
        let cleanup_value = Rc::new(Cell::new(0));
        let late_access_rejected_for_cleanup = late_access_rejected.clone();
        let cleanup_value_for_cleanup = cleanup_value.clone();
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");

        {
            let scope = root.scope();
            let owner = ScopedMountOwner::new(scope);
            let token = owner.token();
            let state = token.owner_state(41).expect("owner state should be active");
            assert_eq!(
                state
                    .with(|value| *value)
                    .expect("state should be readable"),
                41
            );

            owner
                .on_cleanup(
                    Box::new(move || {
                        late_access_rejected_for_cleanup.set(state.with(|_| ()).is_err());
                        if let Some(value) = state.take_for_cleanup() {
                            cleanup_value_for_cleanup.set(value);
                        }
                        Ok(())
                    }),
                    scope
                        .error_handler(|_| {})
                        .expect("error handler should register"),
                )
                .expect("cleanup should register");
        }

        root.dispose().expect("root cleanup should succeed");
        assert!(late_access_rejected.get());
        assert_eq!(cleanup_value.get(), 41);
    }
}
