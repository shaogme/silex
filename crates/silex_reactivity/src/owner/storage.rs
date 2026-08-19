use crate::{
    ReactiveError,
    error::ErrorSlotOwner,
    root::{CleanupFailure, CloseError, ClosePhase, CloseSource, CloseTransaction},
    runtime::OwnerMode,
    runtime::storage::{AllocationCounters, TypedSlotAllocation},
    runtime::{self, CloseReportQueue, GlobalScheduler, OwnerId, ScopeState, run_global_queue},
    unsafe_boundary::{ErasedScopeState, OwnerToken},
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::{Rc, Weak},
};

pub(crate) struct ChildRegistry {
    entries: RefCell<Vec<ChildEntry>>,
}

struct ChildEntry {
    owner_id: OwnerId,
    storage: Rc<ScopeStorage>,
}

struct ParentLink {
    registry: Weak<ChildRegistry>,
}

impl ChildRegistry {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self {
            entries: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn insert(&self, storage: Rc<ScopeStorage>) {
        self.entries.borrow_mut().push(ChildEntry {
            owner_id: storage.owner_id,
            storage,
        });
    }

    pub(crate) fn snapshot(&self) -> Vec<Rc<ScopeStorage>> {
        self.entries
            .borrow()
            .iter()
            .map(|entry| entry.storage.clone())
            .collect()
    }

    pub(crate) fn contains(&self, owner_id: OwnerId, storage: &Rc<ScopeStorage>) -> bool {
        self.entries
            .borrow()
            .iter()
            .any(|entry| entry.owner_id == owner_id && Rc::ptr_eq(&entry.storage, storage))
    }

    fn remove(&self, owner_id: OwnerId) {
        self.entries
            .borrow_mut()
            .retain(|entry| entry.owner_id != owner_id);
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn len(&self) -> usize {
        self.entries.borrow().len()
    }
}

/// Internal result that distinguishes a terminal release from a retryable
/// close error. A terminal release may still carry cleanup diagnostics.
pub(crate) struct CloseOutcome {
    pub(crate) released: bool,
    pub(crate) error: Option<CloseError>,
}

impl CloseOutcome {
    fn released(error: Option<CloseError>) -> Self {
        Self {
            released: true,
            error,
        }
    }

    fn retryable(error: CloseError) -> Self {
        Self {
            released: false,
            error: Some(error),
        }
    }
}

/// Stable storage for one lexical scope and its lifetime-bound payloads.
pub(crate) struct ScopeStorage {
    pub(crate) owner_id: OwnerId,
    pub(crate) state: Rc<ErasedScopeState>,
    pub(crate) allocations: Rc<AllocationCounters>,
    pub(crate) children: Rc<ChildRegistry>,
    parent_link: RefCell<Option<ParentLink>>,
    close_reports: Rc<CloseReportQueue>,
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
        let close_reports = scheduler.borrow().close_reports.clone();
        let state = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let owner_id = scheduler.borrow_mut().alloc_owner(&state, parent, mode);
        state.borrow_mut().owner_id = owner_id;
        Self {
            owner_id,
            state: state.into_inner(),
            allocations: Rc::new(AllocationCounters::new()),
            children: ChildRegistry::new(),
            parent_link: RefCell::new(None),
            close_reports,
        }
    }

    pub(crate) fn link_parent(&self, registry: &Rc<ChildRegistry>) {
        *self.parent_link.borrow_mut() = Some(ParentLink {
            registry: Rc::downgrade(registry),
        });
    }

    pub(crate) fn unlink_parent(&self) {
        let link = self.parent_link.borrow_mut().take();
        if let Some(link) = link.and_then(|link| link.registry.upgrade()) {
            link.remove(self.owner_id);
        }
    }

    pub(crate) fn report_close_error(&self, error: CloseError) {
        self.close_reports.push(error);
    }

    pub(crate) fn alloc_slot<'scope, T: 'scope>(
        &'scope self,
        value: T,
    ) -> TypedSlotAllocation<'scope, T> {
        TypedSlotAllocation::new(Some(value), self.allocations.clone())
    }

    pub(crate) fn alloc_empty_slot<'scope, T: 'scope>(
        &'scope self,
    ) -> TypedSlotAllocation<'scope, T> {
        TypedSlotAllocation::new(None, self.allocations.clone())
    }

    pub(crate) fn alloc_error_slot<'scope, E: 'scope>(&'scope self) -> ErrorSlotOwner<'scope, E> {
        ErrorSlotOwner::new(self.allocations.clone())
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn live_allocations(&self) -> (usize, usize) {
        (
            self.allocations.typed_slots.get(),
            self.allocations.error_slots.get(),
        )
    }

    pub(crate) fn owner_token<'scope>(&'scope self) -> OwnerToken<'scope> {
        OwnerToken::from_storage(self)
    }

    pub(crate) fn dispose(&self) -> CloseOutcome {
        let mut transaction = CloseTransaction::new();
        let scheduler = match self.state.try_borrow() {
            Ok(state) => state.scheduler.clone(),
            Err(_) => {
                transaction.push(
                    ClosePhase::Runtime,
                    CloseSource::Owner,
                    CleanupFailure::Runtime(ReactiveError::BorrowConflict),
                );
                return CloseOutcome::retryable(transaction.finish().expect("close error"));
            }
        };

        let already_released = self
            .state
            .try_borrow()
            .is_ok_and(|state| state.phase == runtime::ScopePhase::Released);
        if already_released {
            let released = scheduler
                .try_borrow_mut()
                .map(|mut scheduler| {
                    scheduler.release_owner_id(self.owner_id);
                    scheduler.owner_metadata(self.owner_id).is_none()
                })
                .unwrap_or(false);
            if released {
                self.unlink_parent();
                return CloseOutcome::released(None);
            }
            transaction.push(
                ClosePhase::Runtime,
                CloseSource::Owner,
                CleanupFailure::Runtime(ReactiveError::BorrowConflict),
            );
            return CloseOutcome::retryable(transaction.finish().expect("close error"));
        }

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
            return transaction.finish().map_or_else(
                || {
                    CloseOutcome::retryable(CloseError::from_panic(Box::new(
                        "owner close did not produce a diagnostic",
                    )))
                },
                CloseOutcome::retryable,
            );
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

        let error = transaction.finish();
        let released = ready_for_release
            && scheduler
                .try_borrow()
                .is_ok_and(|scheduler| scheduler.owner_metadata(self.owner_id).is_none());
        if released {
            self.unlink_parent();
            CloseOutcome::released(error)
        } else {
            CloseOutcome::retryable(error.unwrap_or_else(|| {
                CloseError::from_panic(Box::new(
                    "owner close did not satisfy the release invariant",
                ))
            }))
        }
    }

    pub(crate) fn dispose_untracked(&self) -> CloseOutcome {
        let scheduler = match self.state.try_borrow() {
            Ok(state) => state.scheduler.clone(),
            Err(_) => {
                return CloseOutcome::retryable(
                    CloseError::from_failures(vec![CleanupFailure::Runtime(
                        ReactiveError::BorrowConflict,
                    )])
                    .expect("a borrow conflict must produce a close error"),
                );
            }
        };
        let frame = runtime::ObserverFrame::push(scheduler, None);
        let result = self.dispose();
        drop(frame);
        result
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

    #[cfg(feature = "test-support")]
    pub(crate) fn retained_children(&self) -> usize {
        self.children.len()
    }
}
