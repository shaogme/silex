use crate::{ErrorReporter, Rx, Scope, SilexResult, reactivity::ReactiveSource, traits::RxRead};

/// Create a typed derived node in an explicit scope.
pub trait Map: RxRead + Clone {
    fn map<'scope, U, F>(
        self,
        scope: Scope<'scope>,
        f: F,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        F: Fn(&Self::Value) -> U + 'scope;

    fn map_fn<'scope, U>(
        self,
        scope: Scope<'scope>,
        f: fn(&Self::Value) -> U,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope;
}

impl<S> Map for S
where
    S: RxRead + Clone,
{
    fn map<'scope, U, F>(
        self,
        scope: Scope<'scope>,
        f: F,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        F: Fn(&Self::Value) -> U + 'scope,
    {
        let source = scope.promote(self, error_handler)?;
        scope.derived_from(
            source.runtime_inputs(),
            move || source.with(|value| f(value)),
            error_handler,
        )
    }

    fn map_fn<'scope, U>(
        self,
        scope: Scope<'scope>,
        f: fn(&Self::Value) -> U,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
    {
        let source = scope.promote(self, error_handler)?;
        scope.derived_from(
            source.runtime_inputs(),
            move || source.with(f),
            error_handler,
        )
    }
}

pub trait Memoize: RxRead + Clone {
    fn memo<'scope>(
        self,
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, Self::Value>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope;
}

impl<S> Memoize for S
where
    S: RxRead + Clone,
{
    fn memo<'scope>(
        self,
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, Self::Value>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope,
    {
        let source = scope.promote(self, error_handler)?;
        scope.derived_from(source.runtime_inputs(), move || source.get(), error_handler)
    }
}
