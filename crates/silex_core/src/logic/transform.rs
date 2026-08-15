use crate::{
    ErrorHandlerInput, Rx, Scope, SilexResult, reactivity::ReactiveSource, traits::RxRead,
};

/// Create a typed derived node in an explicit scope.
pub trait Map: RxRead + Clone {
    fn map<'scope, U, F, H>(
        self,
        scope: Scope<'scope>,
        f: F,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        F: Fn(&Self::Value) -> U + 'scope,
        H: ErrorHandlerInput<'scope>;

    fn map_fn<'scope, U, H>(
        self,
        scope: Scope<'scope>,
        f: fn(&Self::Value) -> U,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        H: ErrorHandlerInput<'scope>;
}

impl<S> Map for S
where
    S: RxRead + Clone,
{
    fn map<'scope, U, F, H>(
        self,
        scope: Scope<'scope>,
        f: F,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        F: Fn(&Self::Value) -> U + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self, error_handler)?;
        scope.derived(move || source.with(|value| f(value)), error_handler)
    }

    fn map_fn<'scope, U, H>(
        self,
        scope: Scope<'scope>,
        f: fn(&Self::Value) -> U,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self, error_handler)?;
        scope.derived(move || source.with(f), error_handler)
    }
}

pub trait Memoize: RxRead + Clone {
    fn memo<'scope, H>(
        self,
        scope: Scope<'scope>,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, Self::Value>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;
}

impl<S> Memoize for S
where
    S: RxRead + Clone,
{
    fn memo<'scope, H>(
        self,
        scope: Scope<'scope>,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, Self::Value>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self, error_handler)?;
        scope
            .memo(move |_| source.get(), error_handler)
            .map(|memo| memo.into_rx())
    }
}
