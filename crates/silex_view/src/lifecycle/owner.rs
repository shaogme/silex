use super::{
    context::MountOwnerContext,
    types::{MountCleanup, MountErrorLease},
};
use silex_core::{
    CloseError, ClosePhase, CloseSource, CloseTransaction, EffectHandle, SilexError, SilexErrorKind,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

pub(crate) struct EffectEntry<'scope> {
    pub(crate) handle: EffectHandle<'scope>,
    pub(crate) close_handler: MountErrorLease<'scope>,
}

pub(crate) struct CleanupEntry<'scope> {
    pub(crate) cleanup: MountCleanup<'scope>,
    pub(crate) close_handler: MountErrorLease<'scope>,
}

pub(crate) struct LocalOwnerState<'scope> {
    pub(crate) context: Rc<MountOwnerContext<'scope>>,
    pub(crate) active: Cell<bool>,
    closed: Cell<bool>,
    close_error: RefCell<Option<CloseError>>,
    pub(crate) effects: RefCell<Vec<EffectEntry<'scope>>>,
    pub(crate) cleanups: RefCell<Vec<CleanupEntry<'scope>>>,
    pub(crate) children: RefCell<Vec<Rc<LocalOwnerState<'scope>>>>,
    pub(crate) cleanup_registered: Cell<bool>,
    reported: Cell<bool>,
}

impl<'scope> LocalOwnerState<'scope> {
    pub(crate) fn new(context: Rc<MountOwnerContext<'scope>>) -> Self {
        Self {
            context,
            active: Cell::new(true),
            closed: Cell::new(false),
            close_error: RefCell::new(None),
            effects: RefCell::new(Vec::new()),
            cleanups: RefCell::new(Vec::new()),
            children: RefCell::new(Vec::new()),
            cleanup_registered: Cell::new(false),
            reported: Cell::new(false),
        }
    }

    pub(crate) fn close(&self) -> Result<(), CloseError> {
        if self.closed.get() {
            return self.close_error.borrow().clone().map_or(Ok(()), Err);
        }
        self.active.set(false);
        let mut transaction = CloseTransaction::new();
        for child in self.children.borrow_mut().drain(..).rev() {
            match catch_unwind(AssertUnwindSafe(|| child.close())) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    transaction.push_error(ClosePhase::Child, CloseSource::Child, error)
                }
                Err(panic) => transaction.push_error(
                    ClosePhase::Child,
                    CloseSource::Child,
                    CloseError::from_panic(panic),
                ),
            }
        }
        if self.context.owns_runtime_handles() {
            for entry in self.effects.borrow_mut().drain(..).rev() {
                match catch_unwind(AssertUnwindSafe(|| entry.handle.stop())) {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        let close_error = CloseError::from_panic(Box::new(format!(
                            "View effect stop failed: {error}"
                        )));
                        transaction.push_error(
                            ClosePhase::Effect,
                            CloseSource::Effect,
                            close_error.clone(),
                        );
                        let result = catch_unwind(AssertUnwindSafe(|| {
                            entry
                                .close_handler
                                .handle(SilexError::fatal(SilexErrorKind::Close(close_error)))
                        }));
                        match result {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => transaction.push_error(
                                ClosePhase::Effect,
                                CloseSource::Handler,
                                CloseError::from_panic(Box::new(format!(
                                    "close handler failed: {error}"
                                ))),
                            ),
                            Err(panic) => transaction.push_error(
                                ClosePhase::Effect,
                                CloseSource::Handler,
                                CloseError::from_panic(panic),
                            ),
                        }
                    }
                    Err(panic) => transaction.push_error(
                        ClosePhase::Effect,
                        CloseSource::Effect,
                        CloseError::from_panic(panic),
                    ),
                }
            }
        } else {
            self.effects.borrow_mut().clear();
        }
        for entry in self.cleanups.borrow_mut().drain(..).rev() {
            let result = catch_unwind(AssertUnwindSafe(entry.cleanup));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    let dispatch =
                        catch_unwind(AssertUnwindSafe(|| entry.close_handler.handle(error)));
                    match dispatch {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => transaction.push_error(
                            ClosePhase::Cleanup,
                            CloseSource::Handler,
                            CloseError::from_panic(Box::new(format!(
                                "cleanup handler failed: {error}"
                            ))),
                        ),
                        Err(panic) => transaction.push_error(
                            ClosePhase::Cleanup,
                            CloseSource::Handler,
                            CloseError::from_panic(panic),
                        ),
                    }
                }
                Err(panic) => transaction.push_error(
                    ClosePhase::Cleanup,
                    CloseSource::Cleanup,
                    CloseError::from_panic(panic),
                ),
            }
        }
        let result = transaction.finish();
        self.closed.set(true);
        self.close_error.replace(result.clone());
        result.map_or(Ok(()), Err)
    }

    pub(crate) fn report(&self, error: CloseError) {
        if !self.reported.replace(true)
            && let Some(reporter) = &self.context.cleanup_reporter
        {
            reporter(error);
        }
    }
}
