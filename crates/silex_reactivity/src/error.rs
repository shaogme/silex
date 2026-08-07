//! Explicit runtime operation errors.

use std::{cell::RefCell, fmt, rc::Rc};

/// A response to an operation that cannot be completed in the current scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactiveError {
    /// The handle's scope has already ended or the node was disposed.
    NoSuchNode,
    /// A typed operation reached a node of another internal kind.
    WrongKind,
    /// The node contains a different Rust type than the operation requested.
    TypeMismatch,
    /// A read or write lease conflicts with another lease on the same node.
    BorrowConflict,
    /// A computation or callback is being entered recursively.
    Reentrant,
    /// A runtime was asked to start another run while it was already running.
    RuntimeAlreadyRunning,
    /// Reactive nodes from different scheduler families were combined.
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

/// A scoped, single-threaded destination for callback errors.
///
/// The handler is deliberately an `Fn` so the runtime never holds a mutable
/// borrow into user state while dispatching an error. Callers that need
/// mutable state can capture an `Rc<RefCell<_>>` (or another scoped cell).
pub struct ErrorHandler<'scope, E> {
    callback: Rc<dyn Fn(E) + 'scope>,
}

impl<E> Clone for ErrorHandler<'_, E> {
    fn clone(&self) -> Self {
        Self {
            callback: self.callback.clone(),
        }
    }
}

impl<'scope, E: 'scope> ErrorHandler<'scope, E> {
    /// Create an error handler from a scoped callback.
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(E) + 'scope,
    {
        Self {
            callback: Rc::new(handler),
        }
    }

    /// Create a handler that intentionally discards its input.
    pub fn ignore() -> Self {
        Self::new(|_| {})
    }

    /// Dispatch one error to this handler.
    pub fn handle(&self, error: E) {
        (self.callback)(error);
    }
}

/// Distinguishes registration failures from errors returned by the first
/// callback execution.
#[derive(Debug)]
pub enum EffectInitError<E> {
    /// The runtime could not register or initialize the computation.
    Registration(ReactiveError),
    /// The computation was registered, but its first callback returned this
    /// user error. The provisional computation has already been disposed.
    Initial(E),
}

pub type EffectInitResult<T, E> = Result<T, EffectInitError<E>>;

#[derive(Clone, Copy)]
pub(crate) enum ErrorPhase {
    Initial,
    Deferred,
}

/// Type-erased transport for a scoped callback error.
///
/// The event owns exactly one dispatch operation. Its payload remains erased
/// inside the closure, while the registration adapter retains a typed slot for
/// the initial error result.
pub(crate) struct ErrorEvent<'scope> {
    dispatch: Option<Box<dyn FnOnce(ErrorPhase) + 'scope>>,
}

impl<'scope> ErrorEvent<'scope> {
    pub(crate) fn new<E: 'scope>(
        error: E,
        handler: ErrorHandler<'scope, E>,
        initial_slot: InitialErrorSlot<E>,
    ) -> Self {
        let mut error = Some(error);
        Self {
            dispatch: Some(Box::new(move |phase| {
                let error = error.take().expect("callback error event dispatched twice");
                match phase {
                    ErrorPhase::Initial => initial_slot.store(error),
                    ErrorPhase::Deferred => handler.handle(error),
                }
            })),
        }
    }

    pub(crate) fn deferred<E: 'scope>(error: E, handler: ErrorHandler<'scope, E>) -> Self {
        let mut error = Some(error);
        Self {
            dispatch: Some(Box::new(move |_| {
                let error = error.take().expect("callback error event dispatched twice");
                handler.handle(error);
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

pub(crate) struct InitialErrorSlot<E> {
    value: Rc<RefCell<Option<E>>>,
}

impl<E> Clone for InitialErrorSlot<E> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

impl<E> InitialErrorSlot<E> {
    pub(crate) fn new() -> Self {
        Self {
            value: Rc::new(RefCell::new(None)),
        }
    }

    pub(crate) fn store(&self, error: E) {
        *self.value.borrow_mut() = Some(error);
    }

    pub(crate) fn take(&self) -> E {
        self.value
            .borrow_mut()
            .take()
            .expect("initial callback error slot was not populated")
    }
}
