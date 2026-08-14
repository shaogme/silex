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
}

pub type CallbackInvokeResult<T, E> = Result<T, CallbackInvokeError<E>>;
pub type CompletionSubmitResult<E> = CallbackInvokeResult<bool, E>;

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

/// A typed handler callback. The registry stores only an owner marker; calls
/// always go through the typed `ErrorHandler<E>` capability.
pub(crate) struct ErrorHandlerCell<'scope, E> {
    callback: RefCell<Option<Box<dyn Fn(E) + 'scope>>>,
}

pub(crate) trait ErrorHandlerCall<E> {
    fn call(&self, error: E);
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

    pub(crate) fn call(&self, error: E) {
        let callbacks = self.callback.borrow();
        let callback = callbacks
            .as_ref()
            .expect("error handler callback has already been dropped");
        callback(error);
    }

    pub(crate) fn clear(&self) {
        self.callback.borrow_mut().take();
    }
}

impl<E> ErrorHandlerCall<E> for ErrorHandlerCell<'_, E> {
    fn call(&self, error: E) {
        self.call(error);
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

    pub fn handle(&self, error: E) -> ReactiveResult<()>
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
    dispatch: Option<Box<dyn FnOnce(ErrorPhase) + 'scope>>,
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
                    ErrorPhase::Deferred => {
                        let _ = handler.handle(error);
                    }
                }
            })),
        }
    }

    pub(crate) fn deferred<E: 'scope>(error: E, handler: ErrorHandler<'scope, E>) -> Self {
        let mut error = Some(error);
        Self {
            dispatch: Some(Box::new(move |_| {
                let error = error.take().expect("cleanup error event dispatched twice");
                let _ = handler.handle(error);
            })),
        }
    }

    pub(crate) fn dispatch(mut self, phase: ErrorPhase) {
        let dispatch = self
            .dispatch
            .take()
            .expect("callback error event dispatched twice");
        dispatch(phase);
    }
}
