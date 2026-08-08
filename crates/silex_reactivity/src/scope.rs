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
        let scheduler = {
            let mut state = self.state.borrow_mut();
            if !state.active {
                return;
            }
            state.active = false;
            state.scheduler.clone()
        };
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
        let clear_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.state.borrow_mut().clear_error_handlers();
        }));
        let release_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scheduler.borrow_mut().release_scope_id(self.scope_id);
        }));
        let mut first_panic = dispose_result.err();
        if let Err(panic) = flush_result
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
        if let Err(panic) = clear_result
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
        if let Err(panic) = release_result
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
        if let Some(panic) = first_panic {
            std::panic::resume_unwind(panic);
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

    pub(crate) fn is_active(&self) -> bool {
        let state = match self.state.try_borrow() {
            Ok(state) => state,
            Err(_) => return false,
        };
        state.is_active()
    }
}
