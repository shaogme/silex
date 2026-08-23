//! Explicit runtime operation errors and typed callback error channels.

use crate::{
    borrow::{BorrowCell, BorrowFailure, BorrowSite},
    owner::{EffectPhase, ScopeStorage},
    root::CloseError,
    runtime::{
        ScopePhase, acquire_error_handler_lease, invoke_error_handler,
        storage::{AllocationCounters, AllocationKind, AllocationLease, CallbackThunkError},
    },
    unsafe_boundary::{ActiveOwnerProof, ScopedPtr, WeakOwnerToken},
};
use std::{cell::Cell, fmt, marker::PhantomData, rc::Rc};

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
    DuplicateTarget,
    Handler(HandlerError),
    NonConvergent {
        iterations: usize,
        last_scope: Option<u32>,
        last_node: Option<u64>,
        last_phase: Option<EffectPhase>,
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
            Self::DuplicateTarget => "同一个响应式目标不能在同一事务中重复注册",
            Self::Handler(error) => return error.fmt(f),
            Self::NonConvergent { .. } => "响应式 effect 队列在预算内未收敛",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ReactiveError {}

pub type ReactiveResult<T> = Result<T, ReactiveError>;

impl From<BorrowFailure> for ReactiveError {
    fn from(failure: BorrowFailure) -> Self {
        let _site = failure.site();
        Self::BorrowConflict
    }
}

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
    value: BorrowCell<Option<E>>,
}

impl<E> ErrorSlot<E> {
    pub(crate) fn new() -> Self {
        Self {
            value: BorrowCell::new(None, BorrowSite::Payload),
        }
    }

    pub(crate) fn store(&self, error: E) -> ReactiveResult<()> {
        *self
            .value
            .try_write()
            .map_err(|_| ReactiveError::BorrowConflict)? = Some(error);
        Ok(())
    }

    pub(crate) fn take(&self) -> ReactiveResult<E> {
        self.value
            .try_write()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .take()
            .ok_or(ReactiveError::InvariantViolation)
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
            slot: ScopedPtr::from_ref(&self.inner.slot),
            marker: PhantomData,
        }
    }

    pub(crate) fn store(&self, error: E) -> ReactiveResult<()> {
        self.inner.slot.store(error)
    }

    pub(crate) fn take(&self) -> ReactiveResult<E> {
        self.inner.slot.take()
    }
}

/// A copyable, non-owning reference to a computation error slot.
pub(crate) struct ErrorSlotRef<'scope, E> {
    slot: ScopedPtr<ErrorSlot<E>>,
    marker: PhantomData<fn(&'scope ()) -> &'scope E>,
}

impl<E> Copy for ErrorSlotRef<'_, E> {}

impl<E> Clone for ErrorSlotRef<'_, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, E> ErrorSlotRef<'scope, E> {
    pub(crate) fn identity(self) -> ScopedPtr<()> {
        self.slot.cast()
    }

    pub(crate) fn pointer(self) -> ScopedPtr<ErrorSlot<E>> {
        self.slot
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
    callback: BorrowCell<Option<ErrorHandlerCallback<'scope, E>>>,
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
            callback: BorrowCell::new(Some(Box::new(callback)), BorrowSite::Handler),
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

    pub(crate) fn identity(&self) -> ScopedPtr<()> {
        ScopedPtr::from_ref(self).cast()
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
        let (owner_id, scheduler) = match state.try_borrow() {
            Ok(state) => (state.owner_id, state.scheduler.clone()),
            Err(_) => return false,
        };
        let scheduler = match scheduler.try_borrow() {
            Ok(scheduler) => scheduler,
            Err(_) => return false,
        };
        let state = match scheduler.resolve_cleanup_owner(owner_id) {
            Ok(Some(proof)) => proof.state(),
            Ok(None) | Err(_) => return false,
        };
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
        record: Rc<HandlerRecord<'scope, E>>,
    ) -> HandlerLease<'scope, E> {
        HandlerLease {
            inner: Rc::new(HandlerLeaseInner { owner, record }),
            marker: PhantomData,
        }
    }
}

impl<E> Drop for HandlerRecord<'_, E> {
    fn drop(&mut self) {
        if let Ok(mut callback) = self.callback.try_write() {
            let callback = callback.take();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(callback)));
        }
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
    pub(crate) identity: ScopedPtr<()>,
}

/// A copyable, non-owning dispatch capability for callback errors.
pub struct ErrorHandlerRef<'scope, E> {
    storage: &'scope ScopeStorage,
    key: ErrorHandlerKey,
    record: ScopedPtr<()>,
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
            record: ScopedPtr::from_rc(record).cast(),
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

        let proof = ActiveOwnerProof::from_state(&state)
            .map_err(|error| HandlerError::new(error, ErrorContext::new("handler proof")))?;
        let record = proof
            .clone_handler_record(&state, self.key, self.record.cast())
            .map_err(|error| HandlerError::new(error, ErrorContext::new("handler record")))?;
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

    pub(crate) const fn record(&self) -> ScopedPtr<()> {
        self.record
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

struct HandlerLeaseInner<'scope, E> {
    owner: Rc<dyn HandlerOwner + 'scope>,
    record: Rc<HandlerRecord<'scope, E>>,
}

impl<E> Drop for HandlerLeaseInner<'_, E> {
    fn drop(&mut self) {
        self.owner.release_lease();
    }
}

pub struct HandlerLease<'scope, E> {
    inner: Rc<HandlerLeaseInner<'scope, E>>,
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
        self.inner
            .record
            .call(error, ErrorContext::new("handler callback"), true)
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
    pub(crate) fn invariant(phase: &'static str) -> Self {
        Self {
            dispatch: Some(Box::new(move |_| {
                Err(HandlerError::new(
                    ReactiveError::InvariantViolation,
                    ErrorContext::new(phase),
                ))
            })),
        }
    }

    pub(crate) fn new<E: 'scope>(
        error: E,
        handler: &HandlerLease<'scope, E>,
        slot: ErrorSlotOwner<'scope, E>,
    ) -> Self {
        let handler = handler.clone();
        let mut error = Some(error);
        Self {
            dispatch: Some(Box::new(move |phase| {
                let Some(error) = error.take() else {
                    return Err(HandlerError::new(
                        ReactiveError::InvariantViolation,
                        ErrorContext::new("error event"),
                    ));
                };
                match phase {
                    ErrorPhase::Initial | ErrorPhase::Read => {
                        slot.store(error).map_err(|error| {
                            HandlerError::new(error, ErrorContext::new("error slot"))
                        })?
                    }
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
                let Some(error) = error.take() else {
                    return Err(HandlerError::new(
                        ReactiveError::InvariantViolation,
                        ErrorContext::new("deferred error event"),
                    ));
                };
                handler.handle(error)
            })),
        }
    }

    pub(crate) fn dispatch(mut self, phase: ErrorPhase) -> Result<(), HandlerError> {
        let Some(dispatch) = self.dispatch.take() else {
            return Err(HandlerError::new(
                ReactiveError::InvariantViolation,
                ErrorContext::new("error event dispatch"),
            ));
        };
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
