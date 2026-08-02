use crate::runtime::{self, GlobalScheduler, ScopeId, ScopeState, run_global_queue};
use std::{cell::RefCell, rc::Rc};

/// Stable per-scope metadata referenced by copyable handles.
pub(crate) struct ScopeFrame<'scope> {
    pub(crate) scope_id: ScopeId,
    pub(crate) state: Rc<RefCell<ScopeState<'scope>>>,
}

impl<'scope> ScopeFrame<'scope> {
    pub(crate) fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        let state = Rc::new(RefCell::new(ScopeState::new(ScopeId(0), scheduler.clone())));
        let scope_id = scheduler.borrow_mut().alloc_scope(&state);
        state.borrow_mut().scope_id = scope_id;
        Self { scope_id, state }
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
        match (dispose_result, flush_result) {
            (Err(panic), _) => std::panic::resume_unwind(panic),
            (Ok(()), Err(panic)) => std::panic::resume_unwind(panic),
            (Ok(()), Ok(())) => {}
        }
    }
}
