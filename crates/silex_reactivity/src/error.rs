//! Explicit runtime operation errors and typed callback error channels.

use crate::{
    runtime::invoke_error_handler, runtime::storage::CallbackThunkError, scope::ScopeStorage,
};
use std::{cell::RefCell, fmt, marker::PhantomData};

slotmap::new_key_type! {
    pub(crate) struct ErrorHandlerKey;
}

/// A response to an operation that cannot be completed in the current scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactiveError {
    NoSuchNode,
    WrongKind,
    TypeMismatch,
    BorrowConflict,
    Reentrant,
    RuntimeAlreadyRunning,
    RuntimeMismatch,
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
        matches!(self, Self::WrongKind | Self::TypeMismatch | Self::Reentrant)
    }
}

impl fmt::Display for ReactiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoSuchNode => "节点不存在或所属 scope 已结束",
            Self::WrongKind => "句柄指向的节点不是这个操作要求的种类",
            Self::TypeMismatch => "节点里存放的不是请求的类型",
            Self::BorrowConflict => "同一节点上的动态借用发生冲突",
            Self::Reentrant => "响应式计算或回调发生递归调用",
            Self::RuntimeAlreadyRunning => "响应式 Runtime 已经在运行中",
            Self::RuntimeMismatch => "响应式节点属于不同的 Runtime scheduler family",
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
pub type CompletionSubmitResult<E> = CallbackInvokeResult<bool, E>;

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
enum HandlerReason {
    BorrowConflict,
    NoSuchNode,
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

    pub fn reason(&self) -> ReactiveError {
        match self.reason {
            HandlerReason::BorrowConflict => ReactiveError::BorrowConflict,
            HandlerReason::NoSuchNode => ReactiveError::NoSuchNode,
            HandlerReason::Internal => ReactiveError::TypeMismatch,
        }
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
        write!(f, "错误 handler dispatch 失败：{}", self.reason())
    }
}

impl std::error::Error for HandlerError {}

pub(crate) fn map_callback_error<E>(error: CallbackThunkError<E>) -> CallbackInvokeError<E> {
    match error {
        CallbackThunkError::Runtime(error) => CallbackInvokeError::Runtime(error),
        CallbackThunkError::User(error) => CallbackInvokeError::User(error),
    }
}

/// A typed error slot owned by the scope arena.
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

pub(crate) type ErrorHandlerCallback<'scope, E> = Box<dyn Fn(E) + 'scope>;
pub(crate) type ErrorDispatchCallback<'scope> =
    Box<dyn FnOnce(ErrorPhase) -> Result<(), HandlerError> + 'scope>;

/// A typed handler callback. The registry stores only an owner marker; calls
/// always go through the typed `ErrorHandler<E>` capability.
pub(crate) struct ErrorHandlerCell<'scope, E> {
    callback: RefCell<Option<ErrorHandlerCallback<'scope, E>>>,
}

pub(crate) trait ErrorHandlerCall<E> {
    fn call(&self, error: E) -> ReactiveResult<()>;
}

impl<'scope, E> ErrorHandlerCell<'scope, E> {
    pub(crate) fn new<F>(callback: F) -> Self
    where
        F: Fn(E) + 'scope,
    {
        Self {
            callback: RefCell::new(Some(Box::new(callback))),
        }
    }

    pub(crate) fn call(&self, error: E) -> ReactiveResult<()> {
        let callback = self
            .callback
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .take()
            .ok_or(ReactiveError::NoSuchNode)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(error)));
        let mut callbacks = self
            .callback
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        *callbacks = Some(callback);
        match result {
            Ok(()) => Ok(()),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    pub(crate) fn clear(&self) {
        self.callback.borrow_mut().take();
    }
}

impl<E> ErrorHandlerCall<E> for ErrorHandlerCell<'_, E> {
    fn call(&self, error: E) -> ReactiveResult<()> {
        self.call(error)
    }
}

pub(crate) trait HandlerOwner {
    fn clear(&self);
}

impl<E> HandlerOwner for ErrorHandlerCell<'_, E> {
    fn clear(&self) {
        self.clear();
    }
}

pub(crate) struct ErrorHandlerEntry<'scope> {
    pub(crate) owner: &'scope dyn HandlerOwner,
}

/// A copyable, scoped destination for callback errors.
pub struct ErrorHandler<'scope, E> {
    storage: &'scope ScopeStorage,
    key: ErrorHandlerKey,
    callback: &'scope dyn ErrorHandlerCall<E>,
    marker: PhantomData<fn(E)>,
}

impl<E> Copy for ErrorHandler<'_, E> {}

impl<E> Clone for ErrorHandler<'_, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, E> ErrorHandler<'scope, E> {
    pub(crate) fn from_parts(
        storage: &'scope ScopeStorage,
        key: ErrorHandlerKey,
        callback: &'scope dyn ErrorHandlerCall<E>,
    ) -> Self {
        Self {
            storage,
            key,
            callback,
            marker: PhantomData,
        }
    }

    pub fn handle(&self, error: E) -> Result<(), HandlerError>
    where
        E: 'scope,
    {
        invoke_error_handler(self.storage, self.key, self.callback, error)
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
        handler: ErrorHandler<'scope, E>,
        slot: &'scope ErrorSlot<E>,
    ) -> Self {
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

    pub(crate) fn deferred<E: 'scope>(error: E, handler: ErrorHandler<'scope, E>) -> Self {
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
