//! High-level runtime and owner wrappers.

use crate::{
    Callback, CompletionOnce, CompletionSender, ErrorHandlerInput, ErrorHandlerToken, NodeRef, Rx,
    ScopedSlot, SilexError, SilexErrorKind, SilexResult, TaskHandle,
    reactivity::{
        Computed, EffectHandle, EffectPhase, ReactiveSource, ReadSignal, Signal, StoredValue,
        Transaction, WatchOptions, WriteSignal,
    },
    task,
    traits::{RuntimeScoped, RxData, RxGet},
};
#[cfg(feature = "test-support")]
use silex_reactivity::RuntimeSnapshot;
use silex_reactivity::{
    CloseError, ComputationInitError, OwnerCleanupRegistrationError as ReactiveOwnerCleanupError,
    ReactiveError,
};
use std::{future::Future, marker::PhantomData, panic::UnwindSafe, pin::Pin};

/// User-owned high-level runtime.
pub struct Runtime {
    inner: silex_reactivity::Runtime,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            inner: silex_reactivity::Runtime::new(),
        }
    }

    pub fn owner(&mut self) -> SilexResult<OwnerHandle> {
        self.inner
            .owner()
            .map(|inner| OwnerHandle { inner })
            .map_err(SilexError::fatal)
    }

    pub fn with_transient<R>(
        &mut self,
        f: impl for<'owner> FnOnce(OwnerAccess<'owner>) -> R,
    ) -> SilexResult<R> {
        self.inner
            .with_transient(|owner| f(OwnerAccess { inner: owner }))
            .map_err(SilexError::fatal)
    }

    /// Take close diagnostics produced by drop and panic-recovery paths.
    pub fn take_unhandled_close_errors(&self) -> SilexResult<Vec<CloseError>> {
        self.inner
            .take_unhandled_close_errors()
            .map_err(SilexError::fatal)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent owner with explicit close authority.
pub struct OwnerHandle {
    inner: silex_reactivity::OwnerHandle,
}

impl OwnerHandle {
    pub fn access(&self) -> OwnerAccess<'_> {
        OwnerAccess {
            inner: self.inner.access(),
        }
    }

    pub fn with_access<R>(&self, f: impl FnOnce(OwnerAccess<'_>) -> R) -> R {
        self.inner
            .with_access(|owner| f(OwnerAccess { inner: owner }))
    }

    /// Run a future while retaining the borrowed owner capability.
    pub async fn with_access_async<R>(
        &self,
        f: impl for<'owner> FnOnce(OwnerAccess<'owner>) -> Pin<Box<dyn Future<Output = R> + 'owner>>,
    ) -> R {
        f(self.access()).await
    }

    pub fn create_child(&self) -> SilexResult<Self> {
        self.inner
            .create_child()
            .map(|inner| Self { inner })
            .map_err(SilexError::fatal)
    }

    pub fn close(&self) -> Result<(), CloseError> {
        self.inner.close()
    }

    pub fn is_active(&self) -> SilexResult<bool> {
        self.inner.is_active().map_err(SilexError::fatal)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> SilexResult<RuntimeSnapshot> {
        self.inner.runtime_snapshot().map_err(SilexError::fatal)
    }
}

/// Owner-bound child capability with explicit close authority.
pub struct OwnerChild<'owner> {
    inner: silex_reactivity::OwnerChild<'owner>,
}

/// Error returned when an owner-root cleanup registration cannot accept its
/// payload. The payload is returned unchanged for explicit rollback.
pub struct OwnerCleanupRegistrationError<'owner, T> {
    error: SilexError,
    payload: T,
    marker: PhantomData<&'owner ()>,
}

impl<'owner, T> OwnerCleanupRegistrationError<'owner, T> {
    pub fn into_parts(self) -> (SilexError, T) {
        (self.error, self.payload)
    }
}

impl<'owner> OwnerChild<'owner> {
    /// Borrow the typed access for this owner-bound child.
    pub fn access(&self) -> OwnerAccess<'owner> {
        OwnerAccess {
            inner: self.inner.access(),
        }
    }

    pub fn close(&self) -> Result<(), CloseError> {
        self.inner.close()
    }

    pub fn is_active(&self) -> SilexResult<bool> {
        self.inner.is_active().map_err(SilexError::fatal)
    }
}

/// Borrowed owner capability used to create and operate typed nodes.
#[derive(Clone, Copy)]
pub struct OwnerAccess<'owner> {
    pub(crate) inner: silex_reactivity::OwnerAccess<'owner>,
}

impl PartialEq for OwnerAccess<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for OwnerAccess<'_> {}

impl<'owner> OwnerAccess<'owner> {
    pub fn error_handler<F>(&self, handler: F) -> SilexResult<ErrorHandlerToken<'owner>>
    where
        F: Fn(SilexError) + 'owner,
    {
        self.inner.error_handler(handler).map_err(SilexError::fatal)
    }

    pub fn create_child(&self) -> SilexResult<OwnerHandle> {
        self.inner
            .create_child()
            .map(|inner| OwnerHandle { inner })
            .map_err(SilexError::fatal)
    }

    /// Create an owner-bound child capability.
    pub fn create_owned_child(&self) -> SilexResult<OwnerChild<'owner>> {
        self.inner
            .create_owned_child()
            .map(|inner| OwnerChild { inner })
            .map_err(SilexError::fatal)
    }

    pub fn with_transient<R>(
        &self,
        f: impl for<'child> FnOnce(OwnerAccess<'child>) -> R,
    ) -> SilexResult<R> {
        self.inner
            .with_transient(|owner| f(OwnerAccess { inner: owner }))
            .map_err(SilexError::fatal)
    }

    pub fn is_active(&self) -> SilexResult<bool> {
        self.inner.is_active().map_err(SilexError::fatal)
    }

    /// Run a staged multi-signal transaction bound to this owner.
    pub fn transaction<R, F>(&self, f: F) -> SilexResult<R>
    where
        F: FnOnce(&mut Transaction<'owner>) -> SilexResult<R> + 'owner,
    {
        let inner = self
            .inner
            .reactive_transaction()
            .map_err(map_transaction_error)?;
        let mut transaction = Transaction::from_inner(inner);
        match f(&mut transaction) {
            Ok(value) => transaction.commit().map(|()| value),
            Err(error) => {
                drop(transaction);
                Err(error)
            }
        }
    }

    /// Validate a reactive source before creating target-side nodes.
    pub fn validate_runtime<S>(&self, source: &S) -> SilexResult<()>
    where
        S: RuntimeScoped + ?Sized,
    {
        if !self.is_active()? {
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        if self
            .inner
            .same_runtime(&source.owner_access().inner)
            .map_err(SilexError::fatal)?
        {
            Ok(())
        } else {
            Err(SilexError::fatal(ReactiveError::RuntimeMismatch))
        }
    }

    pub fn signal<T: 'owner>(&self, value: T) -> SilexResult<Signal<'owner, T>> {
        let signal = self.inner.signal(value).map_err(SilexError::fatal)?;
        let (read, write) = signal.into_pair();
        Signal::from_pair((
            ReadSignal::from_inner(read, *self),
            WriteSignal::from_inner(write, *self),
        ))
    }

    pub fn computed<T, F, H>(&self, f: F, error_handler: H) -> SilexResult<Computed<'owner, T>>
    where
        T: PartialEq + 'owner,
        F: FnMut() -> SilexResult<T> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        self.inner
            .computed(f, error_handler)
            .map(|computed| Computed::from_inner(computed, *self))
            .map_err(map_computation_error)
    }

    pub fn computed_always<T, F, H>(
        &self,
        f: F,
        error_handler: H,
    ) -> SilexResult<Computed<'owner, T>>
    where
        T: 'owner,
        F: FnMut() -> SilexResult<T> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        self.inner
            .computed_always(f, error_handler)
            .map(|computed| Computed::from_inner(computed, *self))
            .map_err(map_computation_error)
    }

    pub fn effect<F, H>(
        &self,
        phase: EffectPhase,
        f: F,
        error_handler: H,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        F: FnMut() -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.inner
            .effect(phase, f, error_handler.handler_ref())
            .map(EffectHandle::from_inner)
            .map_err(map_computation_error)
    }

    /// Register a framework-owned effect detached from the current
    /// computation tree. The callback's own children and cleanups remain
    /// owned by the detached effect and are stopped with its handle.
    #[doc(hidden)]
    pub fn effect_detached<F, H>(
        &self,
        phase: EffectPhase,
        f: F,
        error_handler: H,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        F: FnMut() -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.inner
            .effect_detached(phase, f, error_handler.handler_ref())
            .map(EffectHandle::from_inner)
            .map_err(map_computation_error)
    }

    pub fn effect_with_previous<T, F, H>(
        &self,
        phase: EffectPhase,
        f: F,
        error_handler: H,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        T: 'owner,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.inner
            .effect_with_previous(phase, f, error_handler.handler_ref())
            .map(EffectHandle::from_inner)
            .map_err(map_computation_error)
    }

    pub fn watch<S, C, H>(
        &self,
        phase: EffectPhase,
        source: S,
        callback: C,
        error_handler: H,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        S: ReactiveSource<'owner>,
        S::Owned: Sized + Clone + PartialEq + RxData + 'owner,
        C: FnMut(&S::Owned, Option<&S::Owned>) -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.watch_with_options(
            phase,
            source,
            callback,
            error_handler,
            WatchOptions::default(),
        )
    }

    pub fn watch_with_options<S, C, H>(
        &self,
        phase: EffectPhase,
        source: S,
        callback: C,
        error_handler: H,
        options: WatchOptions,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        S: ReactiveSource<'owner>,
        S::Owned: Sized + Clone + PartialEq + RxData + 'owner,
        C: FnMut(&S::Owned, Option<&S::Owned>) -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = source
            .into_promotion_plan()
            .materialize(*self, error_handler)?;
        self.watch_getter_with_options(
            phase,
            move || source.get(),
            callback,
            error_handler,
            options,
        )
    }

    pub fn watch_getter<T, G, C, H>(
        &self,
        phase: EffectPhase,
        getter: G,
        callback: C,
        error_handler: H,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        T: PartialEq + 'owner,
        G: FnMut() -> SilexResult<T> + 'owner,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.watch_getter_with_options(
            phase,
            getter,
            callback,
            error_handler,
            WatchOptions::default(),
        )
    }

    pub fn watch_getter_with_options<T, G, C, H>(
        &self,
        phase: EffectPhase,
        getter: G,
        callback: C,
        error_handler: H,
        options: WatchOptions,
    ) -> SilexResult<EffectHandle<'owner>>
    where
        T: PartialEq + 'owner,
        G: FnMut() -> SilexResult<T> + 'owner,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.inner
            .watch_getter_with_options(
                phase,
                getter,
                callback,
                error_handler.handler_ref(),
                options,
            )
            .map(EffectHandle::from_inner)
            .map_err(map_computation_error)
    }

    pub fn stored<T: 'owner>(&self, value: T) -> SilexResult<StoredValue<'owner, T>> {
        self.inner
            .stored(value)
            .map(|stored| StoredValue::from_inner(stored, *self))
            .map_err(SilexError::fatal)
    }

    pub fn callback<T, F>(&self, callback: F) -> SilexResult<Callback<'owner, T>>
    where
        T: 'owner,
        F: FnMut(T) -> SilexResult<()> + 'owner,
    {
        self.inner
            .callback(callback)
            .map(Callback::from_inner)
            .map_err(SilexError::fatal)
    }

    pub fn node_ref<T: 'owner>(&self) -> SilexResult<NodeRef<'owner, T>> {
        self.inner
            .node_ref()
            .map(NodeRef::from_inner)
            .map_err(SilexError::fatal)
    }

    pub fn scoped_slot<T: 'owner>(&self) -> SilexResult<ScopedSlot<'owner, T>> {
        self.inner
            .node_ref()
            .map(ScopedSlot::from_inner)
            .map_err(SilexError::fatal)
    }

    pub fn completion_once<T, F>(&self, callback: F) -> SilexResult<CompletionOnce<T>>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'owner,
    {
        self.inner
            .completion_once(callback)
            .map_err(SilexError::fatal)
    }

    pub fn completion_sender<T, F>(&self, callback: F) -> SilexResult<CompletionSender<T>>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'owner,
    {
        self.inner
            .completion_sender(callback)
            .map_err(SilexError::fatal)
    }

    #[doc(hidden)]
    pub fn completion_once_detached<T, F>(&self, callback: F) -> SilexResult<CompletionOnce<T>>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'owner,
    {
        self.inner
            .completion_once_detached(callback)
            .map_err(SilexError::fatal)
    }

    #[doc(hidden)]
    pub fn completion_sender_detached<T, F>(&self, callback: F) -> SilexResult<CompletionSender<T>>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'owner,
    {
        self.inner
            .completion_sender_detached(callback)
            .map_err(SilexError::fatal)
    }

    pub fn spawn_scoped<F, H>(&self, future: F, error_handler: H) -> SilexResult<TaskHandle<'owner>>
    where
        F: Future<Output = ()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        if !self.is_active()? {
            return Ok(TaskHandle::inactive());
        }
        let (task, cancel) = task::start(future);
        let cleanup = self.on_cleanup(
            move || {
                cancel();
                Ok::<(), SilexError>(())
            },
            error_handler,
        );
        match cleanup {
            Ok(()) => Ok(task),
            Err(error) => {
                task.cancel();
                Err(error)
            }
        }
    }

    pub fn promote<T, H>(&self, value: T, error_handler: H) -> SilexResult<Rx<'owner, T::Owned>>
    where
        T: ReactiveSource<'owner>,
        T::Owned: Sized + RxData + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        value
            .into_promotion_plan()
            .materialize(*self, error_handler.handler_ref())
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> SilexResult<RuntimeSnapshot> {
        self.inner.runtime_snapshot().map_err(SilexError::fatal)
    }

    pub fn constant<T: 'owner>(&self, value: T) -> SilexResult<Rx<'owner, T>> {
        let stored = self.stored(value)?;
        Ok(Rx::from_stored(stored))
    }

    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> SilexResult<R> {
        self.inner.untrack(f).map_err(SilexError::fatal)
    }

    #[doc(hidden)]
    pub fn with_runtime<R>(&self, f: impl FnOnce() -> R) -> SilexResult<R> {
        self.inner.with_runtime(f).map_err(SilexError::fatal)
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> SilexResult<R> {
        self.inner.batch(f).map_err(SilexError::fatal)
    }

    pub fn on_cleanup<F, H>(&self, f: F, error_handler: H) -> SilexResult<()>
    where
        F: FnOnce() -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.inner
            .on_cleanup(f, error_handler.handler_ref())
            .map_err(SilexError::fatal)
    }

    pub fn on_owner_cleanup<T, F, H>(
        &self,
        payload: T,
        cleanup: F,
        error_handler: H,
    ) -> Result<(), OwnerCleanupRegistrationError<'owner, T>>
    where
        T: 'owner,
        F: FnOnce(T) -> SilexResult<()> + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        self.inner
            .on_owner_cleanup(payload, cleanup, error_handler.handler_ref())
            .map_err(|error: ReactiveOwnerCleanupError<'owner, T>| {
                let (error, payload) = error.into_parts();
                OwnerCleanupRegistrationError {
                    error: SilexError::fatal(error),
                    payload,
                    marker: PhantomData,
                }
            })
    }
}

fn map_computation_error(error: ComputationInitError<SilexError>) -> SilexError {
    match error {
        ComputationInitError::Registration(error) => SilexError::fatal(error),
        ComputationInitError::Initial(error) => error,
    }
}

fn map_transaction_error(error: silex_reactivity::TransactionError) -> SilexError {
    SilexError::fatal(SilexErrorKind::Transaction(Box::new(error)))
}
