use crate::{
    ErrorHandlerInput, OwnerAccess, Rx, SilexResult, reactivity::ReactiveSource, traits::RxRead,
};

/// Create a typed computed node in an explicit owner.
pub trait Map: RxRead + Clone {
    fn map<'scope, U, F, H>(
        self,
        owner: OwnerAccess<'scope>,
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
        owner: OwnerAccess<'scope>,
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
        owner: OwnerAccess<'scope>,
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
        let source = owner.promote(self, error_handler)?;
        owner
            .computed_always(move || source.with(|value| f(value)), error_handler)
            .map(crate::Computed::into_rx)
    }

    fn map_fn<'scope, U, H>(
        self,
        owner: OwnerAccess<'scope>,
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
        let source = owner.promote(self, error_handler)?;
        owner
            .computed_always(move || source.with(f), error_handler)
            .map(crate::Computed::into_rx)
    }
}

pub trait ComputedSource: RxRead + Clone {
    fn computed<'scope, H>(
        self,
        owner: OwnerAccess<'scope>,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, Self::Value>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;
}

impl<S> ComputedSource for S
where
    S: RxRead + Clone,
{
    fn computed<'scope, H>(
        self,
        owner: OwnerAccess<'scope>,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, Self::Value>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self, error_handler)?;
        owner
            .computed(move || source.get(), error_handler)
            .map(|computed| computed.into_rx())
    }
}
