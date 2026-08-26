use silex_core::{SilexError, SilexErrorKind, SilexResult};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

type RowCallback<'scope, T> = Box<dyn FnMut(T, usize) -> SilexResult<()> + 'scope>;

struct RowUpdaterState<'scope, T> {
    generation: Cell<u64>,
    callback: RefCell<Option<RowCallback<'scope, T>>>,
}

/// 带 generation 的 owner-bound row update capability。
pub struct RowUpdater<'scope, T> {
    state: Rc<RowUpdaterState<'scope, T>>,
    generation: u64,
}

impl<'scope, T> Clone for RowUpdater<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            generation: self.generation,
        }
    }
}

impl<'scope, T> RowUpdater<'scope, T> {
    pub(crate) fn new() -> Self {
        Self {
            state: Rc::new(RowUpdaterState {
                generation: Cell::new(0),
                callback: RefCell::new(None),
            }),
            generation: 0,
        }
    }

    pub fn bind<F>(&self, callback: F) -> bool
    where
        F: FnMut(T, usize) -> SilexResult<()> + 'scope,
    {
        if !self.is_generation_active() {
            return false;
        }
        let mut slot = self.state.callback.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(Box::new(callback));
        true
    }

    pub fn update(&self, item: T, index: usize) -> SilexResult<()> {
        if !self.is_generation_active() {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "stateful row updater is no longer active".into(),
            )));
        }
        let Some(mut callback) = self.state.callback.borrow_mut().take() else {
            return Err(SilexError::fatal(SilexErrorKind::Framework(
                "stateful row updater has no callback".into(),
            )));
        };
        let result = catch_unwind(AssertUnwindSafe(|| callback(item, index)));
        if self.is_generation_active() && self.state.callback.borrow().is_none() {
            *self.state.callback.borrow_mut() = Some(callback);
        }
        match result {
            Ok(result) => result,
            Err(panic) => resume_unwind(panic),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_generation_active() && self.state.callback.borrow().is_some()
    }

    fn is_generation_active(&self) -> bool {
        self.state.generation.get() == self.generation
    }

    pub(crate) fn invalidate(&self) {
        if self.is_generation_active() {
            self.state.generation.set(self.generation.wrapping_add(1));
        }
        let _ = self.state.callback.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use super::RowUpdater;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn stale_updater_is_inert_and_reentrant_dispatch_is_safe() {
        let calls = Rc::new(Cell::new(0));
        let updater = RowUpdater::new();
        let stale = updater.clone();
        let reentrant = updater.clone();
        let calls_for_callback = calls.clone();
        assert!(updater.bind(move |_, _| {
            calls_for_callback.set(calls_for_callback.get() + 1);
            assert!(reentrant.update(2, 0).is_err());
            Ok(())
        }));
        assert!(stale.update(1, 0).is_ok());
        updater.invalidate();
        assert!(stale.update(2, 0).is_err());
        assert_eq!(calls.get(), 1);
    }
}
