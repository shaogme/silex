use super::{
    token::MountOwnerToken,
    types::{MountCleanup, MountEffect, MountErrorHandler},
};
use silex_core::{EffectPhase, OwnerChild, SilexResult};

/// View mount 时 owner 所需的最小能力。
pub trait MountOwner<'scope> {
    fn effect(
        &self,
        phase: EffectPhase,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;
    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;
    fn token(&self) -> MountOwnerToken<'scope>;
    fn child(&self) -> MountOwnerToken<'scope>;
    fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, MountOwnerToken<'scope>)>;
}

impl<'scope> MountOwner<'scope> for MountOwnerToken<'scope> {
    fn effect(
        &self,
        phase: EffectPhase,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        MountOwnerToken::effect(self, phase, callback, error_handler)
    }

    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        MountOwnerToken::on_cleanup(self, cleanup, error_handler)
    }

    fn token(&self) -> MountOwnerToken<'scope> {
        self.clone()
    }

    fn child(&self) -> MountOwnerToken<'scope> {
        MountOwnerToken::child(self)
    }

    fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, MountOwnerToken<'scope>)> {
        MountOwnerToken::branch_child(self)
    }
}

pub(crate) struct OwnerMount<'scope> {
    token: MountOwnerToken<'scope>,
}

impl<'scope> OwnerMount<'scope> {
    pub(crate) fn new(token: MountOwnerToken<'scope>) -> Self {
        Self { token }
    }

    pub(crate) fn token(&self) -> MountOwnerToken<'scope> {
        self.token.clone()
    }
}

impl<'scope> MountOwner<'scope> for OwnerMount<'scope> {
    fn effect(
        &self,
        phase: EffectPhase,
        callback: MountEffect<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.token.effect(phase, callback, error_handler)
    }

    fn on_cleanup(
        &self,
        cleanup: MountCleanup<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.token.on_cleanup(cleanup, error_handler)
    }

    fn token(&self) -> MountOwnerToken<'scope> {
        self.token.clone()
    }

    fn child(&self) -> MountOwnerToken<'scope> {
        self.token.child()
    }

    fn branch_child(&self) -> SilexResult<(OwnerChild<'scope>, MountOwnerToken<'scope>)> {
        self.token.branch_child()
    }
}

#[cfg(test)]
mod tests {
    use super::MountOwnerToken;
    use silex_core::{Runtime, SilexError, SilexErrorKind};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn cleanup_runs_in_owner_scope_and_dispatches_errors() {
        let observed = Rc::new(Cell::new(0));
        let observed_by_handler = observed.clone();
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|access| {
                let handler = access
                    .error_handler(move |_| observed_by_handler.set(observed_by_handler.get() + 1))
                    .expect("handler");
                let owner = MountOwnerToken::new(access);
                owner
                    .on_cleanup(
                        Box::new(|| {
                            Err(SilexError::recoverable(SilexErrorKind::Framework(
                                "cleanup".into(),
                            )))
                        }),
                        handler.view(),
                    )
                    .expect("cleanup");
                owner.close().expect("close should dispatch");
            })
            .expect("transient scope");
        assert_eq!(observed.get(), 1);
    }
}
