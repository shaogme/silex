use crate::{
    ErrorHandlerInput, Memo, Rx, Scope, SilexResult, reactivity::ReactiveSource, traits::RxRead,
};

fn compare<'scope, A, B, F, H>(
    scope: Scope<'scope>,
    left: A,
    right: B,
    compare: F,
    error_handler: H,
) -> SilexResult<Rx<'scope, bool>>
where
    A: ReactiveSource<'scope> + 'scope,
    B: ReactiveSource<'scope> + 'scope,
    A::Value: Sized + PartialEq + 'scope,
    B::Value: Sized + PartialEq + 'scope,
    F: Fn(&A::Value, &B::Value) -> bool + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    let error_handler = error_handler.handler_ref();
    let left = left.into_promotion_plan();
    let right = right.into_promotion_plan();
    let left = left.materialize(scope, error_handler)?;
    let right = right.materialize(scope, error_handler)?;
    scope
        .memo(
            move |_| left.with(|left| right.with(|right| compare(left, right)))?,
            error_handler,
        )
        .map(Memo::into_rx)
}

pub trait ReactivePartialEq: RxRead + Clone {
    fn equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;

    fn not_equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone,
{
    fn equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left == right,
            error_handler,
        )
    }

    fn not_equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left != right,
            error_handler,
        )
    }
}

pub trait ReactivePartialOrd: RxRead + Clone {
    fn greater_than<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;

    fn less_than<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;

    fn greater_than_or_equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;

    fn less_than_or_equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>;
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone,
{
    fn greater_than<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left > right,
            error_handler,
        )
    }

    fn less_than<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left < right,
            error_handler,
        )
    }

    fn greater_than_or_equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left >= right,
            error_handler,
        )
    }

    fn less_than_or_equals<'scope, O, H>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left <= right,
            error_handler,
        )
    }
}
