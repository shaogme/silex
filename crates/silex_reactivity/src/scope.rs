use crate::{
    ReactiveError,
    root::{CleanupError, CleanupFailure},
    runtime::{self, GlobalScheduler, ScopeId, ScopeState, run_global_queue},
    unsafe_boundary::{ErasedScopeState, OwnerToken},
};
use std::{
    cell::RefCell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

/// Stable storage for one lexical scope and its lifetime-bound payloads.
pub(crate) struct ScopeStorage {
    pub(crate) scope_id: ScopeId,
    pub(crate) state: Rc<ErasedScopeState>,
    pub(crate) arena: bumpalo::Bump,
}

impl ScopeStorage {
    pub(crate) fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        let state = ScopeState::new(ScopeId(0), scheduler.clone());
        let scope_id = scheduler.borrow_mut().alloc_scope(&state);
        state.borrow_mut().scope_id = scope_id;
        Self {
            scope_id,
            state: state.into_inner(),
            arena: bumpalo::Bump::new(),
        }
    }

    pub(crate) fn alloc_slot<'scope, T: 'scope>(
        &'scope self,
        value: T,
    ) -> crate::runtime::storage::TypedNodeRef<'scope, T> {
        crate::runtime::storage::TypedNodeRef::from_slot(
            self.arena
                .alloc(crate::runtime::storage::TypedSlot::new(value)),
        )
    }

    pub(crate) fn alloc_empty_slot<'scope, T: 'scope>(
        &'scope self,
    ) -> crate::runtime::storage::TypedNodeRef<'scope, T> {
        crate::runtime::storage::TypedNodeRef::from_slot(
            self.arena
                .alloc(crate::runtime::storage::TypedSlot::empty()),
        )
    }

    pub(crate) fn alloc_error_slot<'scope, E: 'scope>(
        &'scope self,
    ) -> &'scope crate::error::ErrorSlot<E> {
        self.arena.alloc(crate::error::ErrorSlot::new())
    }

    pub(crate) fn owner_token<'scope>(
        &self,
        owner: PhantomData<fn(&'scope ()) -> &'scope ()>,
    ) -> OwnerToken<'scope> {
        OwnerToken::from_storage(self.state.clone(), owner)
    }

    pub(crate) fn dispose(&self) -> Result<(), CleanupError> {
        let mut failures = Vec::new();
        let scheduler = match self.state.try_borrow() {
            Ok(state) => state.scheduler.clone(),
            Err(_) => {
                failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
                return match CleanupError::from_failures(failures) {
                    Some(error) => Err(error),
                    None => Ok(()),
                };
            }
        };

        let should_dispose = match self.state.try_borrow_mut() {
            Ok(mut state) => match state.begin_quiescing() {
                Ok(value) => value,
                Err(error) => {
                    failures.push(CleanupFailure::Runtime(error));
                    false
                }
            },
            Err(_) => {
                failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
                false
            }
        };
        if !should_dispose {
            return CleanupError::from_failures(failures).map_or(Ok(()), Err);
        }

        if scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .map(|mut scheduler| scheduler.deactivate_scope(self.scope_id))
            .is_err()
        {
            failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
        }

        let typed_state = self.owner_token(PhantomData).state();
        let report = catch_unwind(AssertUnwindSafe(|| runtime::dispose_all(&typed_state)));
        match report {
            Ok(report) => {
                failures.extend(
                    report
                        .runtime_errors
                        .into_iter()
                        .map(CleanupFailure::Runtime),
                );
                failures.extend(
                    report
                        .handler_errors
                        .into_iter()
                        .map(CleanupFailure::Handler),
                );
                failures.extend(report.panics.into_iter().map(CleanupError::panic_failure));
            }
            Err(panic) => failures.push(CleanupError::panic_failure(panic)),
        }

        let ready_for_release = match self.state.try_borrow() {
            Ok(state) => state.ready_for_scope_release(),
            Err(_) => {
                failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
                false
            }
        };
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.sweep_error_handlers();
        }
        if ready_for_release {
            let handlers = match self.state.try_borrow_mut() {
                Ok(mut state) => Some(state.take_error_handlers()),
                Err(_) => {
                    failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
                    None
                }
            };
            if let Some(handlers) = handlers {
                let panics = ScopeState::drop_error_handlers(handlers);
                failures.extend(panics.into_iter().map(CleanupError::panic_failure));
            }
            if scheduler
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)
                .map(|mut scheduler| scheduler.release_scope_id(self.scope_id))
                .is_err()
            {
                failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
            }
        }

        let should_flush = match scheduler.try_borrow() {
            Ok(scheduler) => scheduler.should_flush(),
            Err(_) => {
                failures.push(CleanupFailure::Runtime(ReactiveError::BorrowConflict));
                false
            }
        };
        if should_flush {
            match run_global_queue(&scheduler) {
                Ok(()) => {}
                Err(error) => failures.push(CleanupFailure::Runtime(error)),
            }
        }

        CleanupError::from_failures(failures).map_or(Ok(()), Err)
    }

    pub(crate) fn dispose_untracked(&self) -> Result<(), CleanupError> {
        let frame = runtime::ObserverFrame::push(self.scheduler(), None);
        let result = self.dispose();
        drop(frame);
        result
    }

    pub(crate) fn scheduler(&self) -> Rc<RefCell<GlobalScheduler>> {
        self.state.borrow().scheduler.clone()
    }

    pub(crate) fn is_active(&self) -> bool {
        let state = match self.state.try_borrow() {
            Ok(state) => state,
            Err(_) => return false,
        };
        state.is_active()
    }
}
