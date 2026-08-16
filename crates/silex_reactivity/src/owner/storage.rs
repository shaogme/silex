use crate::{
    ReactiveError,
    error::ErrorSlot,
    root::{CleanupFailure, CloseError, ClosePhase, CloseSource, CloseTransaction},
    runtime::OwnerMode,
    runtime::storage::{TypedNodeRef, TypedSlot},
    runtime::{self, GlobalScheduler, OwnerId, ScopeState, run_global_queue},
    unsafe_boundary::{ErasedScopeState, OwnerToken},
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

/// Stable storage for one lexical scope and its lifetime-bound payloads.
pub(crate) struct ScopeStorage {
    pub(crate) owner_id: OwnerId,
    pub(crate) state: Rc<ErasedScopeState>,
    pub(crate) arena: bumpalo::Bump,
    pub(crate) children: RefCell<Vec<Rc<ScopeStorage>>>,
}

impl ScopeStorage {
    #[cfg(test)]
    pub(crate) fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        Self::new_with_owner(scheduler, None, OwnerMode::Transient)
    }

    pub(crate) fn new_with_owner(
        scheduler: Rc<RefCell<GlobalScheduler>>,
        parent: Option<OwnerId>,
        mode: OwnerMode,
    ) -> Self {
        let state = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let owner_id = scheduler.borrow_mut().alloc_owner(&state, parent, mode);
        state.borrow_mut().owner_id = owner_id;
        Self {
            owner_id,
            state: state.into_inner(),
            arena: bumpalo::Bump::new(),
            children: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn alloc_slot<'scope, T: 'scope>(&'scope self, value: T) -> TypedNodeRef<'scope, T> {
        TypedNodeRef::from_slot(self.arena.alloc(TypedSlot::new(value)))
    }

    pub(crate) fn alloc_empty_slot<'scope, T: 'scope>(&'scope self) -> TypedNodeRef<'scope, T> {
        TypedNodeRef::from_slot(self.arena.alloc(TypedSlot::empty()))
    }

    pub(crate) fn alloc_error_slot<'scope, E: 'scope>(&'scope self) -> &'scope ErrorSlot<E> {
        self.arena.alloc(ErrorSlot::new())
    }

    pub(crate) fn owner_token<'scope>(&'scope self) -> OwnerToken<'scope> {
        OwnerToken::from_storage(self)
    }

    pub(crate) fn dispose(&self) -> Result<(), CloseError> {
        let mut transaction = CloseTransaction::new();
        let scheduler = match self.state.try_borrow() {
            Ok(state) => state.scheduler.clone(),
            Err(_) => {
                transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                );
                return transaction.finish().map_or(Ok(()), Err);
            }
        };

        let should_dispose = match self.state.try_borrow_mut() {
            Ok(mut state) => match state.begin_quiescing() {
                Ok(value) => value,
                Err(error) => {
                    transaction.push(
                        ClosePhase::Runtime,
                        CloseSource::Owner,
                        CleanupFailure::Runtime(error),
                    );
                    false
                }
            },
            Err(_) => {
                transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                );
                false
            }
        };
        if !should_dispose {
            return transaction.finish().map_or(Ok(()), Err);
        }

        if scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .map(|mut scheduler| scheduler.deactivate_scope(self.owner_id))
            .is_err()
        {
            transaction.push(
                ClosePhase::Runtime,
                CloseSource::Owner,
                CleanupFailure::Runtime(ReactiveError::BorrowConflict),
            );
        }

        let typed_state = self.owner_token().state();
        let report = catch_unwind(AssertUnwindSafe(|| runtime::dispose_all(&typed_state)));
        match report {
            Ok(report) => {
                for error in report.runtime_errors {
                    transaction.push(
                        ClosePhase::Runtime,
                        CloseSource::Owner,
                        CleanupFailure::Runtime(error),
                    );
                }
                for error in report.handler_errors {
                    transaction.push(
                        ClosePhase::Cleanup,
                        CloseSource::Handler,
                        CleanupFailure::Handler(error),
                    );
                }
                for panic in report.panics {
                    transaction.push_error(
                        ClosePhase::Cleanup,
                        CloseSource::Cleanup,
                        CloseError::from_panic(panic),
                    );
                }
            }
            Err(panic) => transaction.push_error(
                ClosePhase::Cleanup,
                CloseSource::Cleanup,
                CloseError::from_panic(panic),
            ),
        }

        let ready_for_release = match self.state.try_borrow() {
            Ok(state) => state.ready_for_scope_release(),
            Err(_) => {
                transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                );
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
                    transaction.push(
                        ClosePhase::Runtime,
                        CloseSource::Owner,
                        CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                    );
                    None
                }
            };
            if let Some(handlers) = handlers {
                let panics = ScopeState::drop_error_handlers(handlers);
                for panic in panics {
                    transaction.push_error(
                        ClosePhase::Cleanup,
                        CloseSource::Handler,
                        CloseError::from_panic(panic),
                    );
                }
            }
            if scheduler
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)
                .map(|mut scheduler| scheduler.release_owner_id(self.owner_id))
                .is_err()
            {
                transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                );
            }
        }

        let should_flush = match scheduler.try_borrow() {
            Ok(scheduler) => scheduler.should_flush(),
            Err(_) => {
                transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                );
                false
            }
        };
        if should_flush {
            match run_global_queue(&scheduler) {
                Ok(()) => {}
                Err(error) => transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(error),
                ),
            }
        }

        transaction.finish().map_or(Ok(()), Err)
    }

    pub(crate) fn dispose_untracked(&self) -> Result<(), CloseError> {
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
        let active = state.is_active();
        drop(state);
        active && self.owner_mode().is_some()
    }

    pub(crate) fn owner_mode(&self) -> Option<OwnerMode> {
        self.state.try_borrow().ok().and_then(|state| {
            state
                .scheduler
                .try_borrow()
                .ok()?
                .owner_metadata(state.owner_id)
                .map(|(mode, _parent)| mode)
        })
    }
}
