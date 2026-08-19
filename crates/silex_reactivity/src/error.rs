//! Explicit runtime operation errors and typed callback error channels.

use crate::{
    owner::ScopeStorage,
    root::CloseError,
    runtime::{
        ScopePhase, ScopeState, acquire_error_handler_lease, invoke_error_handler,
        storage::{AllocationCounters, AllocationKind, AllocationLease, CallbackThunkError},
    },
    unsafe_boundary::{OwnerToken, WeakOwnerToken},
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

slotmap::new_key_type! {
    pub(crate) struct ErrorHandlerKey;
}

/// A response to an operation that cannot be completed in the current scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactiveError {
    NoSuchNode,
    WrongKind,
    BorrowConflict,
    Reentrant,
    RuntimeAlreadyRunning,
    RuntimeMismatch,
    InvariantViolation,
    Handler(HandlerError),
    NonConvergent {
        iterations: usize,
        last_scope: Option<u32>,
        last_node: Option<u64>,
    },
}

impl ReactiveError {
    #[inline]
    pub fn is_bug(self) -> bool {
        matches!(
            self,
            Self::WrongKind | Self::Reentrant | Self::InvariantViolation
        )
    }
}

impl fmt::Display for ReactiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoSuchNode => "节点不存在或所属 scope 已结束",
            Self::WrongKind => "句柄指向的节点不是这个操作要求的种类",
            Self::BorrowConflict => "同一节点上的动态借用发生冲突",
            Self::Reentrant => "响应式计算或回调发生递归调用",
            Self::RuntimeAlreadyRunning => "响应式 Runtime 已经在运行中",
            Self::RuntimeMismatch => "响应式节点属于不同的 Runtime scheduler family",
            Self::InvariantViolation => "响应式运行时内部状态不一致",
            Self::Handler(error) => return error.fmt(f),
            Self::NonConvergent { .. } => "响应式 effect 队列在预算内未收敛",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ReactiveError {}

pub type ReactiveResult<T> = Result<T, ReactiveError>;

#[derive(Debug, PartialEq, Eq)]
pub enum CallbackInvokeError<E> {
    Runtime(ReactiveError),
    User(E),
    Handler(HandlerError),
}

pub type CallbackInvokeResult<T, E> = Result<T, CallbackInvokeError<E>>;

/// Errors returned by a completion submission's callback and close phases.
#[derive(Debug, PartialEq, Eq)]
pub enum CompletionSubmitError<E> {
    Callback(CallbackInvokeError<E>),
    Close(Box<CloseError>),
    CallbackAndClose {
        callback: CallbackInvokeError<E>,
        close: Box<CloseError>,
    },
}

impl<E> CompletionSubmitError<E> {
    pub fn callback(&self) -> Option<&CallbackInvokeError<E>> {
        match self {
            Self::Callback(callback) | Self::CallbackAndClose { callback, .. } => Some(callback),
            Self::Close(_) => None,
        }
    }

    pub fn close(&self) -> Option<&CloseError> {
        match self {
            Self::Close(close) | Self::CallbackAndClose { close, .. } => Some(close),
            Self::Callback(_) => None,
        }
    }

    pub fn into_parts(self) -> (Option<CallbackInvokeError<E>>, Option<CloseError>) {
        match self {
            Self::Callback(callback) => (Some(callback), None),
            Self::Close(close) => (None, Some(*close)),
            Self::CallbackAndClose { callback, close } => (Some(callback), Some(*close)),
        }
    }
}

pub type CompletionSubmitResult<E> = Result<bool, CompletionSubmitError<E>>;

/// Additional information attached to an error handler dispatch failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorContext {
    pub owner: Option<u32>,
    pub node_kind: Option<&'static str>,
    pub node_id: Option<u64>,
    pub phase: &'static str,
}

impl ErrorContext {
    pub(crate) const fn new(phase: &'static str) -> Self {
        Self {
            owner: None,
            node_kind: None,
            node_id: None,
            phase,
        }
    }

    pub(crate) const fn with_owner(mut self, owner: u32) -> Self {
        self.owner = Some(owner);
        self
    }
}

/// Failure to deliver a user error to its registered handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandlerError {
    reason: HandlerReason,
    context: ErrorContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandlerReason {
    BorrowConflict,
    NoSuchNode,
    Inactive,
    GenerationMismatch,
    ScopeReleased,
    Internal,
}

impl HandlerError {
    pub(crate) fn new(reason: ReactiveError, context: ErrorContext) -> Self {
        let reason = match reason {
            ReactiveError::BorrowConflict => HandlerReason::BorrowConflict,
            ReactiveError::NoSuchNode => HandlerReason::NoSuchNode,
            ReactiveError::Handler(_) => HandlerReason::Internal,
            _ => HandlerReason::Internal,
        };
        Self { reason, context }
    }

    pub(crate) const fn inactive(context: ErrorContext) -> Self {
        Self {
            reason: HandlerReason::Inactive,
            context,
        }
    }

    pub(crate) const fn generation_mismatch(context: ErrorContext) -> Self {
        Self {
            reason: HandlerReason::GenerationMismatch,
            context,
        }
    }

    pub(crate) const fn scope_released(context: ErrorContext) -> Self {
        Self {
            reason: HandlerReason::ScopeReleased,
            context,
        }
    }

    pub fn reason(&self) -> HandlerReason {
        self.reason
    }

    pub fn context(&self) -> &ErrorContext {
        &self.context
    }

    pub(crate) fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "错误 handler dispatch 失败：{:?}", self.reason())
    }
}

impl std::error::Error for HandlerError {}

pub(crate) fn map_callback_error<E>(error: CallbackThunkError<E>) -> CallbackInvokeError<E> {
    match error {
        CallbackThunkError::Runtime(error) => CallbackInvokeError::Runtime(error),
        CallbackThunkError::User(error) => CallbackInvokeError::User(error),
    }
}

/// A typed error slot owned by the computation and any in-flight error event.
pub(crate) struct ErrorSlot<E> {
    value: RefCell<Option<E>>,
}

impl<E> ErrorSlot<E> {
    pub(crate) fn new() -> Self {
        Self {
            value: RefCell::new(None),
        }
    }

    pub(crate) fn store(&self, error: E) {
        *self.value.borrow_mut() = Some(error);
    }

    pub(crate) fn take(&self) -> E {
        self.value
            .borrow_mut()
            .take()
            .expect("typed error slot was not populated")
    }
}

pub(crate) struct ErrorSlotOwner<'scope, E> {
    inner: Rc<ErrorSlotInner<E>>,
    marker: PhantomData<fn(&'scope ()) -> &'scope E>,
}

struct ErrorSlotInner<E> {
    slot: ErrorSlot<E>,
    _lease: AllocationLease,
}

impl<'scope, E> Clone for ErrorSlotOwner<'scope, E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<'scope, E> ErrorSlotOwner<'scope, E> {
    pub(crate) fn new(counters: Rc<AllocationCounters>) -> Self {
        Self {
            inner: Rc::new(ErrorSlotInner {
                slot: ErrorSlot::new(),
                _lease: AllocationLease::new(counters, AllocationKind::Error),
            }),
            marker: PhantomData,
        }
    }

    pub(crate) fn reference(&self) -> ErrorSlotRef<'scope, E> {
        ErrorSlotRef {
            slot: NonNull::from(&self.inner.slot),
            marker: PhantomData,
        }
    }

    pub(crate) fn store(&self, error: E) {
        self.inner.slot.store(error);
    }

    pub(crate) fn take(&self) -> E {
        self.inner.slot.take()
    }
}

/// A copyable, non-owning reference to a computation error slot.
pub(crate) struct ErrorSlotRef<'scope, E> {
    slot: NonNull<ErrorSlot<E>>,
    marker: PhantomData<fn(&'scope ()) -> &'scope E>,
}

impl<E> Copy for ErrorSlotRef<'_, E> {}

impl<E> Clone for ErrorSlotRef<'_, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, E> ErrorSlotRef<'scope, E> {
    /// Restore the slot after the owning computation has been validated.
    ///
    /// # Safety
    ///
    /// The caller must have just validated that the computation owning this
    /// slot is live and has the expected generation and kind.
    pub(crate) unsafe fn restore(self) -> &'scope ErrorSlot<E> {
        // SAFETY: upheld by the function contract above.
        unsafe { self.slot.as_ref() }
    }
}

pub(crate) type ErrorHandlerCallback<'scope, E> = Box<dyn Fn(E) + 'scope>;
pub(crate) type ErrorDispatchCallback<'scope> =
    Box<dyn FnOnce(ErrorPhase) -> Result<(), HandlerError> + 'scope>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum HandlerState {
    Active,
    Closing,
    Retired,
}

struct InFlightGuard<'a, 'scope, E> {
    record: &'a HandlerRecord<'scope, E>,
}

impl<E> Drop for InFlightGuard<'_, '_, E> {
    fn drop(&mut self) {
        self.record
            .in_flight
            .set(self.record.in_flight.get().saturating_sub(1));
        self.record.maybe_retire();
    }
}

/// A heap-owned typed callback record. The registry keeps a type-erased owner
/// for lifecycle bookkeeping, while all callback invocation remains typed.
pub(crate) struct HandlerRecord<'scope, E> {
    callback: RefCell<Option<ErrorHandlerCallback<'scope, E>>>,
    state: Cell<HandlerState>,
    strong_count: Cell<usize>,
    lease_count: Cell<usize>,
    in_flight: Cell<usize>,
    pending_retire: Cell<bool>,
    owner: WeakOwnerToken,
    key: Cell<Option<ErrorHandlerKey>>,
}

impl<'scope, E> HandlerRecord<'scope, E> {
    pub(crate) fn new<F>(callback: F, owner: WeakOwnerToken) -> Self
    where
        F: Fn(E) + 'scope,
    {
        Self {
            callback: RefCell::new(Some(Box::new(callback))),
            state: Cell::new(HandlerState::Active),
            strong_count: Cell::new(1),
            lease_count: Cell::new(0),
            in_flight: Cell::new(0),
            pending_retire: Cell::new(false),
            owner,
            key: Cell::new(None),
        }
    }

    pub(crate) fn set_key(&self, key: ErrorHandlerKey) {
        self.key.set(Some(key));
    }

    pub(crate) fn identity(&self) -> NonNull<()> {
        NonNull::from(self).cast()
    }

    fn is_active(&self) -> bool {
        self.state.get() == HandlerState::Active
    }

    fn add_strong(&self) {
        self.strong_count
            .set(self.strong_count.get().saturating_add(1));
    }

    fn release_strong(&self) {
        self.strong_count
            .set(self.strong_count.get().saturating_sub(1));
        if self.strong_count.get() == 0 {
            self.begin_closing();
        }
    }

    fn begin_closing(&self) {
        if self.state.get() == HandlerState::Active {
            self.state.set(HandlerState::Closing);
        }
        self.maybe_retire();
    }

    fn add_lease(&self, context: ErrorContext) -> Result<(), HandlerError> {
        if !self.is_active() {
            return Err(Self::inactive_error(context));
        }
        self.lease_count
            .set(self.lease_count.get().saturating_add(1));
        Ok(())
    }

    fn release_lease(&self) {
        self.lease_count
            .set(self.lease_count.get().saturating_sub(1));
        self.maybe_retire();
    }

    fn inactive_error(context: ErrorContext) -> HandlerError {
        match context.phase {
            "handler scope" => HandlerError::scope_released(context),
            _ => HandlerError::inactive(context),
        }
    }

    fn can_dispatch(&self, allow_closing: bool, context: ErrorContext) -> Result<(), HandlerError> {
        match self.state.get() {
            HandlerState::Active => Ok(()),
            HandlerState::Closing if allow_closing && self.lease_count.get() > 0 => Ok(()),
            HandlerState::Closing | HandlerState::Retired => Err(Self::inactive_error(context)),
        }
    }

    pub(crate) fn call(
        &self,
        error: E,
        context: ErrorContext,
        allow_closing: bool,
    ) -> Result<(), HandlerError> {
        self.can_dispatch(allow_closing, context)?;
        self.in_flight.set(self.in_flight.get().saturating_add(1));
        let guard = InFlightGuard { record: self };
        let callback = match self.callback.try_borrow_mut() {
            Ok(mut callback) => match callback.take() {
                Some(callback) => callback,
                None => {
                    drop(guard);
                    return Err(Self::inactive_error(context));
                }
            },
            Err(_) => {
                drop(guard);
                return Err(HandlerError::new(ReactiveError::BorrowConflict, context));
            }
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(error)));
        let restore = self.state.get() == HandlerState::Active
            || (allow_closing && self.lease_count.get() > 0);
        if restore && let Ok(mut callbacks) = self.callback.try_borrow_mut() {
            *callbacks = Some(callback);
        }
        drop(guard);
        match result {
            Ok(()) => Ok(()),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    pub(crate) fn force_retire(&self) {
        self.state.set(HandlerState::Retired);
        let callback_cleared = match self.callback.try_borrow_mut() {
            Ok(mut callback) => {
                let callback = callback.take();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(callback))).is_ok()
            }
            Err(_) => false,
        };
        self.pending_retire.set(!callback_cleared);
        self.strong_count.set(0);
        self.lease_count.set(0);
        self.in_flight.set(0);
    }

    fn maybe_retire(&self) {
        if self.state.get() == HandlerState::Active
            || self.strong_count.get() != 0
            || self.lease_count.get() != 0
            || self.in_flight.get() != 0
        {
            return;
        }
        self.state.set(HandlerState::Retired);
        let callback_cleared = match self.callback.try_borrow_mut() {
            Ok(mut callback) => {
                let callback = callback.take();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(callback))).is_ok()
            }
            Err(_) => false,
        };
        if !callback_cleared || !self.remove_from_registry() {
            self.pending_retire.set(true);
        } else {
            self.pending_retire.set(false);
        }
    }

    fn remove_from_registry(&self) -> bool {
        let Some(key) = self.key.get() else {
            return true;
        };
        let Some(state) = self.owner.upgrade_erased() else {
            return true;
        };
        // SAFETY: this weak identity was captured from the same registered
        // handler owner. The registry entry is removed only after the owner
        // state has finished its close transaction.
        let state: ScopeState<'scope> = unsafe { OwnerToken::from_validated(state).state() };
        if let Ok(mut state) = state.try_borrow_mut() {
            state.remove_error_handler(key, self.identity());
            true
        } else {
            false
        }
    }

    pub(crate) fn lease(
        &self,
        owner: Rc<dyn HandlerOwner + 'scope>,
        pointer: NonNull<()>,
    ) -> HandlerLease<'scope, E> {
        HandlerLease {
            inner: Rc::new(HandlerLeaseInner {
                owner,
                record: pointer.cast(),
            }),
            marker: PhantomData,
        }
    }
}

impl<E> Drop for HandlerRecord<'_, E> {
    fn drop(&mut self) {
        let callback = self.callback.get_mut().take();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(callback)));
    }
}

pub(crate) trait HandlerOwner {
    fn is_active(&self) -> bool;
    fn is_pending_retire(&self) -> bool;
    fn add_lease(&self, context: ErrorContext) -> Result<(), HandlerError>;
    fn release_lease(&self);
    fn force_retire(&self);
}

impl<E> HandlerOwner for HandlerRecord<'_, E> {
    fn is_active(&self) -> bool {
        self.is_active()
    }

    fn is_pending_retire(&self) -> bool {
        self.pending_retire.get()
    }

    fn add_lease(&self, context: ErrorContext) -> Result<(), HandlerError> {
        self.add_lease(context)
    }

    fn release_lease(&self) {
        self.release_lease();
    }

    fn force_retire(&self) {
        self.force_retire();
    }
}

pub(crate) struct ErrorHandlerEntry<'scope> {
    pub(crate) owner: Rc<dyn HandlerOwner + 'scope>,
    pub(crate) identity: NonNull<()>,
}

/// A copyable, non-owning dispatch capability for callback errors.
pub struct ErrorHandlerRef<'scope, E> {
    storage: &'scope ScopeStorage,
    key: ErrorHandlerKey,
    record: NonNull<()>,
    marker: PhantomData<fn(E) -> &'scope ()>,
}

impl<E> Copy for ErrorHandlerRef<'_, E> {}

impl<E> Clone for ErrorHandlerRef<'_, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, E: 'scope> ErrorHandlerRef<'scope, E> {
    pub(crate) fn from_record(
        storage: &'scope ScopeStorage,
        key: ErrorHandlerKey,
        record: &Rc<HandlerRecord<'scope, E>>,
    ) -> Self {
        Self {
            storage,
            key,
            record: NonNull::new(Rc::as_ptr(record).cast_mut())
                .expect("handler record pointer cannot be null")
                .cast(),
            marker: PhantomData,
        }
    }

    pub fn handle(&self, error: E) -> Result<(), HandlerError> {
        invoke_error_handler(self, error)
    }

    pub fn lease(&self) -> Result<HandlerLease<'scope, E>, HandlerError> {
        acquire_error_handler_lease(self)
    }

    #[doc(hidden)]
    pub fn anchor(&self) -> Result<ErrorHandlerAnchor<'scope, E>, HandlerError> {
        let state = self.storage.owner_token().state();
        {
            let state_ref = state.try_borrow().map_err(|error| {
                HandlerError::new(error, ErrorContext::new("handler state lookup"))
            })?;
            let context = ErrorContext::new("handler token").with_owner(state_ref.owner_id.0);
            if state_ref.phase == ScopePhase::Released {
                return Err(HandlerError::scope_released(context));
            }
            let entry = state_ref
                .error_handlers
                .get(self.key)
                .ok_or_else(|| HandlerError::generation_mismatch(context))?;
            if entry.identity != self.record {
                return Err(HandlerError::generation_mismatch(context));
            }
            if !entry.owner.is_active() {
                return Err(HandlerError::inactive(context));
            }
        }

        let pointer = self.record.cast::<HandlerRecord<'scope, E>>().as_ptr();
        // SAFETY: the registry validation above proves that this pointer is
        // the live Rc allocation for the current handler generation.
        unsafe { Rc::increment_strong_count(pointer) };
        // SAFETY: increment_strong_count created exactly one Rc strong ref.
        let record = unsafe { Rc::from_raw(pointer) };
        record.add_strong();
        Ok(ErrorHandlerAnchor::from_record(
            self.storage,
            self.key,
            record,
        ))
    }

    #[doc(hidden)]
    pub fn is_same_handler(&self, other: &Self) -> bool {
        std::ptr::eq(self.storage, other.storage)
            && self.key == other.key
            && self.record == other.record
    }

    pub(crate) fn storage(&self) -> &'scope ScopeStorage {
        self.storage
    }

    pub(crate) const fn key(&self) -> ErrorHandlerKey {
        self.key
    }

    pub(crate) const fn record(&self) -> NonNull<()> {
        self.record
    }

    pub(crate) unsafe fn restore_record(&self) -> &'scope HandlerRecord<'scope, E> {
        // SAFETY: Runtime lookup validates both the generation key and the
        // record identity before this pointer is restored. The record is
        // kept alive by the registry or by the HandlerLease owner.
        unsafe { self.record.cast::<HandlerRecord<'scope, E>>().as_ref() }
    }
}

/// An owning handler reference retained by a framework lifecycle context.
///
/// Unlike [`ErrorHandlerRef`], this value keeps the registered callback alive
/// after the caller drops its [`ErrorHandlerToken`]. It is deliberately
/// separate from the public token so a lifecycle context cannot close the
/// caller's registration accidentally.
pub struct ErrorHandlerAnchor<'scope, E> {
    view: ErrorHandlerRef<'scope, E>,
    record: Rc<HandlerRecord<'scope, E>>,
}

impl<'scope, E: 'scope> ErrorHandlerAnchor<'scope, E> {
    fn from_record(
        storage: &'scope ScopeStorage,
        key: ErrorHandlerKey,
        record: Rc<HandlerRecord<'scope, E>>,
    ) -> Self {
        Self {
            view: ErrorHandlerRef::from_record(storage, key, &record),
            record,
        }
    }

    pub fn view(&self) -> ErrorHandlerRef<'scope, E> {
        self.view
    }
}

impl<E> Clone for ErrorHandlerAnchor<'_, E> {
    fn clone(&self) -> Self {
        self.record.add_strong();
        Self {
            view: self.view,
            record: self.record.clone(),
        }
    }
}

impl<E> Drop for ErrorHandlerAnchor<'_, E> {
    fn drop(&mut self) {
        self.record.release_strong();
    }
}

/// The RAII owner for one registered error callback.
pub struct ErrorHandlerToken<'scope, E> {
    view: ErrorHandlerRef<'scope, E>,
    record: Rc<HandlerRecord<'scope, E>>,
    closed: Cell<bool>,
}

impl<'scope, E> Clone for ErrorHandlerToken<'scope, E> {
    fn clone(&self) -> Self {
        self.record.add_strong();
        Self {
            view: self.view,
            record: self.record.clone(),
            closed: Cell::new(false),
        }
    }
}

impl<'scope, E: 'scope> ErrorHandlerToken<'scope, E> {
    pub(crate) fn from_record(
        storage: &'scope ScopeStorage,
        key: ErrorHandlerKey,
        record: Rc<HandlerRecord<'scope, E>>,
    ) -> Self {
        let view = ErrorHandlerRef::from_record(storage, key, &record);
        Self {
            view,
            record,
            closed: Cell::new(false),
        }
    }

    pub fn view(&self) -> ErrorHandlerRef<'scope, E> {
        self.view
    }

    pub fn handle(&self, error: E) -> Result<(), HandlerError> {
        self.view.handle(error)
    }

    pub fn close(&self) -> Result<(), HandlerError> {
        if !self.closed.replace(true) {
            self.record.begin_closing();
            self.record.release_strong();
        }
        Ok(())
    }
}

impl<E> Drop for ErrorHandlerToken<'_, E> {
    fn drop(&mut self) {
        if !self.closed.get() {
            self.record.release_strong();
        }
    }
}

/// A public input accepted by computation and cleanup APIs.
#[doc(hidden)]
pub trait ErrorHandlerInput<'scope, E> {
    fn handler_ref(&self) -> ErrorHandlerRef<'scope, E>;
}

impl<'scope, E> ErrorHandlerInput<'scope, E> for ErrorHandlerToken<'scope, E> {
    fn handler_ref(&self) -> ErrorHandlerRef<'scope, E> {
        self.view
    }
}

impl<'scope, E> ErrorHandlerInput<'scope, E> for ErrorHandlerAnchor<'scope, E> {
    fn handler_ref(&self) -> ErrorHandlerRef<'scope, E> {
        self.view
    }
}

impl<'scope, E> ErrorHandlerInput<'scope, E> for ErrorHandlerRef<'scope, E> {
    fn handler_ref(&self) -> ErrorHandlerRef<'scope, E> {
        *self
    }
}

impl<'scope, E, T> ErrorHandlerInput<'scope, E> for &T
where
    T: ErrorHandlerInput<'scope, E> + ?Sized,
{
    fn handler_ref(&self) -> ErrorHandlerRef<'scope, E> {
        T::handler_ref(self)
    }
}

struct HandlerLeaseInner<'scope> {
    owner: Rc<dyn HandlerOwner + 'scope>,
    record: NonNull<()>,
}

impl Drop for HandlerLeaseInner<'_> {
    fn drop(&mut self) {
        self.owner.release_lease();
    }
}

pub struct HandlerLease<'scope, E> {
    inner: Rc<HandlerLeaseInner<'scope>>,
    marker: PhantomData<fn(E) -> &'scope ()>,
}

impl<'scope, E> Clone for HandlerLease<'scope, E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<'scope, E> HandlerLease<'scope, E> {
    pub fn handle(&self, error: E) -> Result<(), HandlerError> {
        // SAFETY: The lease owns the type-erased record owner and the pointer
        // was validated against the same registry entry before the lease was
        // created.
        let record = unsafe {
            self.inner
                .record
                .cast::<HandlerRecord<'scope, E>>()
                .as_ref()
        };
        record.call(error, ErrorContext::new("handler callback"), true)
    }
}

#[derive(Debug)]
pub enum ComputationInitError<E> {
    Registration(ReactiveError),
    Initial(E),
}

pub type ComputationInitResult<T, E> = Result<T, ComputationInitError<E>>;

#[derive(Clone, Copy)]
pub(crate) enum ErrorPhase {
    Initial,
    Read,
    Deferred,
}

/// An error event keeps its concrete payload and dispatches it directly to a
/// typed slot or typed handler. No erased value crosses the scheduler.
pub(crate) struct ErrorEvent<'scope> {
    dispatch: Option<ErrorDispatchCallback<'scope>>,
}

impl<'scope> ErrorEvent<'scope> {
    pub(crate) fn new<E: 'scope>(
        error: E,
        handler: &HandlerLease<'scope, E>,
        slot: ErrorSlotOwner<'scope, E>,
    ) -> Self {
        let handler = handler.clone();
        let mut error = Some(error);
        Self {
            dispatch: Some(Box::new(move |phase| {
                let error = error.take().expect("callback error event dispatched twice");
                match phase {
                    ErrorPhase::Initial | ErrorPhase::Read => slot.store(error),
                    ErrorPhase::Deferred => handler.handle(error)?,
                }
                Ok(())
            })),
        }
    }

    pub(crate) fn deferred<E: 'scope>(error: E, handler: &HandlerLease<'scope, E>) -> Self {
        let handler = handler.clone();
        let mut error = Some(error);
        Self {
            dispatch: Some(Box::new(move |_| {
                let error = error.take().expect("cleanup error event dispatched twice");
                handler.handle(error)
            })),
        }
    }

    pub(crate) fn dispatch(mut self, phase: ErrorPhase) -> Result<(), HandlerError> {
        let dispatch = self
            .dispatch
            .take()
            .expect("callback error event dispatched twice");
        dispatch(phase)
    }

    pub(crate) fn dispatch_with_context(
        self,
        phase: ErrorPhase,
        context: ErrorContext,
    ) -> Result<(), HandlerError> {
        self.dispatch(phase)
            .map_err(|error| error.with_context(context))
    }
}
