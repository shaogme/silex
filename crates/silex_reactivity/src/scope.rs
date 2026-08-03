use crate::runtime::{self, GlobalScheduler, ScopeId, ScopeState, run_global_queue};
use std::{cell::RefCell, rc::Rc};

pub(crate) type ErasedScopeState = RefCell<ScopeState<'static>>;

/// Stable storage for one lexical scope and its lifetime-bound payloads.
pub(crate) struct ScopeStorage {
    pub(crate) scope_id: ScopeId,
    pub(crate) state: Rc<ErasedScopeState>,
}

impl ScopeStorage {
    pub(crate) fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        let state: Rc<ErasedScopeState> =
            Rc::new(RefCell::new(ScopeState::new(ScopeId(0), scheduler.clone())));
        let scope_id = scheduler.borrow_mut().alloc_scope(&state);
        state.borrow_mut().scope_id = scope_id;
        Self { scope_id, state }
    }

    /// Restore the payload lifetime owned by the lexical capability.
    ///
    /// # Safety
    ///
    /// The caller must prove that the returned state is used only while the
    /// owning lexical scope is active, and that disposal runs before any data
    /// captured by the scope's payloads becomes invalid. The erased state is
    /// never exposed as a public `'static` state.
    pub(crate) unsafe fn typed_state<'scope>(&self) -> Rc<RefCell<ScopeState<'scope>>> {
        // SAFETY: the caller supplies the lexical lifetime represented by the
        // Scope or OwnedScope capability that owns this storage.
        unsafe {
            std::mem::transmute::<Rc<ErasedScopeState>, Rc<RefCell<ScopeState<'scope>>>>(
                self.state.clone(),
            )
        }
    }

    pub(crate) fn dispose(&self) {
        let scheduler = self.state.borrow().scheduler.clone();
        scheduler.borrow_mut().deactivate_scope(self.scope_id);
        let dispose_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime::dispose_all(&self.state);
        }));
        let flush_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let should_flush = scheduler.borrow().should_flush();
            if should_flush {
                run_global_queue(&scheduler);
            }
        }));
        scheduler.borrow_mut().release_scope_id(self.scope_id);
        match (dispose_result, flush_result) {
            (Err(panic), _) => std::panic::resume_unwind(panic),
            (Ok(()), Err(panic)) => std::panic::resume_unwind(panic),
            (Ok(()), Ok(())) => {}
        }
    }

    pub(crate) fn dispose_untracked(&self) {
        let frame = runtime::ObserverFrame::push(self.scheduler(), None);
        self.dispose();
        drop(frame);
    }

    pub(crate) fn scheduler(&self) -> Rc<RefCell<GlobalScheduler>> {
        self.state.borrow().scheduler.clone()
    }
}
