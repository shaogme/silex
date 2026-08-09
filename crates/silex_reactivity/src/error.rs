//! Explicit runtime operation errors.

use crate::{internal::value::AnyValue, runtime::invoke_error_handler, scope::ScopeStorage};
use std::{cell::RefCell, fmt, marker::PhantomData, rc::Rc};

slotmap::new_key_type! {
    pub(crate) struct ErrorHandlerKey;
}

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

/// Distinguishes runtime failures from errors returned by a user callback.
#[derive(Debug)]
pub enum CallbackInvokeError<E> {
    /// The callback could not be entered because the runtime operation failed.
    Runtime(ReactiveError),
    /// The callback was entered and returned this user-defined error.
    User(E),
}

pub type CallbackInvokeResult<T, E> = Result<T, CallbackInvokeError<E>>;

pub(crate) type ErasedErrorCallback<'scope> = dyn Fn(AnyValue<'scope>) + 'scope;

pub(crate) struct ErrorHandlerEntry<'scope> {
    pub(crate) callback: Rc<ErasedErrorCallback<'scope>>,
}

impl<'scope> ErrorHandlerEntry<'scope> {
    pub(crate) fn new<E, F>(handler: F) -> Self
    where
        E: 'scope,
        F: Fn(E) + 'scope,
    {
        let callback: Rc<ErasedErrorCallback<'scope>> = Rc::new(move |value| {
            let error = unsafe {
                value
                    .downcast::<E>()
                    .expect("error handler payload type must match")
            };
            handler(error);
        });
        Self { callback }
    }
}

/// A copyable, scoped destination for callback errors.
///
/// The callback is owned by the scope registry. Copies of this handle only
/// copy the registry key and never clone callback ownership. The registry
/// keeps the callback until scope disposal completes.
pub struct ErrorHandler<'scope, E> {
    storage: &'scope ScopeStorage,
    key: ErrorHandlerKey,
    marker: PhantomData<fn(E)>,
}

impl<'scope, E> Copy for ErrorHandler<'scope, E> {}

impl<'scope, E> Clone for ErrorHandler<'scope, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, E> ErrorHandler<'scope, E> {
    pub(crate) fn from_parts(storage: &'scope ScopeStorage, key: ErrorHandlerKey) -> Self {
        Self {
            storage,
            key,
            marker: PhantomData,
        }
    }

    /// Dispatch one error and report a stale or invalid registry key.
    pub fn try_handle(&self, error: E) -> ReactiveResult<()>
    where
        E: 'scope,
    {
        invoke_error_handler(self.storage, self.key, error)
    }

    /// Dispatch one error, panicking if the owning scope no longer has the
    /// registered handler.
    pub fn handle(&self, error: E)
    where
        E: 'scope,
    {
        self.try_handle(error)
            .expect("派发 scoped error handler 失败");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Scope, runtime::GlobalScheduler, scope::ScopeStorage};
    use std::{cell::Cell, marker::PhantomData, rc::Rc};

    #[test]
    fn stale_handler_is_rejected_after_disposal_and_scope_id_reuse() {
        let scheduler = GlobalScheduler::new();
        let first_storage = ScopeStorage::new(scheduler.clone());
        let first_scope = Scope {
            storage: &first_storage,
            _marker: PhantomData,
        };
        let first_calls = Cell::new(0);
        let first_handler = first_scope.error_handler(|_: &'static str| {
            first_calls.set(first_calls.get() + 1);
        });
        let first_state = unsafe { first_storage.typed_state() };
        assert_eq!(first_state.borrow().error_handlers.len(), 1);
        assert_eq!(first_state.borrow().nodes.len(), 0);

        first_storage.dispose_untracked();
        assert_eq!(first_state.borrow().error_handlers.len(), 0);
        assert_eq!(
            first_handler.try_handle("stale"),
            Err(ReactiveError::NoSuchNode)
        );
        assert_eq!(first_calls.get(), 0);

        let second_storage = ScopeStorage::new(scheduler);
        let second_scope = Scope {
            storage: &second_storage,
            _marker: PhantomData,
        };
        let second_calls = Cell::new(0);
        let second_handler = second_scope.error_handler(|_: &'static str| {
            second_calls.set(second_calls.get() + 1);
        });

        assert_eq!(
            first_handler.try_handle("still stale"),
            Err(ReactiveError::NoSuchNode)
        );
        second_handler.handle("current");
        assert_eq!(second_calls.get(), 1);

        second_storage.dispose_untracked();
    }

    #[test]
    fn handler_drop_panic_does_not_leave_registry_entries() {
        struct PanicOnDrop(Rc<Cell<usize>>);

        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
                panic!("handler capture drop panic");
            }
        }

        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let drops = Rc::new(Cell::new(0));
        let drop_probe = PanicOnDrop(drops.clone());
        let _handler = scope.error_handler(move |_: ()| {
            let _ = &drop_probe;
        });
        let state = unsafe { storage.typed_state() };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            storage.dispose_untracked();
        }));

        assert!(result.is_err());
        assert_eq!(state.borrow().error_handlers.len(), 0);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn handler_capture_drop_sees_an_already_empty_registry() {
        struct ReentrantDrop<'scope> {
            handler: Rc<RefCell<Option<ErrorHandler<'scope, ()>>>>,
            result: Rc<RefCell<Option<ReactiveResult<()>>>>,
        }

        impl Drop for ReentrantDrop<'_> {
            fn drop(&mut self) {
                let Some(handler) = *self.handler.borrow() else {
                    return;
                };
                *self.result.borrow_mut() = Some(handler.try_handle(()));
            }
        }

        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let saved_handler = Rc::new(RefCell::new(None));
        let result = Rc::new(RefCell::new(None));
        let probe = ReentrantDrop {
            handler: saved_handler.clone(),
            result: result.clone(),
        };
        let handler = scope.error_handler(move |_: ()| {
            let _ = &probe;
        });
        *saved_handler.borrow_mut() = Some(handler);

        storage.dispose_untracked();

        assert_eq!(*result.borrow(), Some(Err(ReactiveError::NoSuchNode)));
    }
}
