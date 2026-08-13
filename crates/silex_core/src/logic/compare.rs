use crate::{
    ErrorReporter, Memo, Rx, Scope, SilexResult, reactivity::ReactiveSource, traits::RxRead,
};

fn compare<'scope, A, B, F>(
    scope: Scope<'scope>,
    left: A,
    right: B,
    compare: F,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<Rx<'scope, bool>>
where
    A: ReactiveSource<'scope> + 'scope,
    B: ReactiveSource<'scope> + 'scope,
    A::Value: Sized + PartialEq + 'scope,
    B::Value: Sized + PartialEq + 'scope,
    F: Fn(&A::Value, &B::Value) -> bool + 'scope,
{
    let left = left.into_promotion_plan();
    let right = right.into_promotion_plan();
    let mut inputs = left.inputs();
    inputs.extend(&right.inputs());
    scope.validate_inputs(&inputs)?;
    let left = left.materialize(scope, error_handler)?;
    let right = right.materialize(scope, error_handler)?;
    scope
        .memo_from(
            inputs,
            move |_| left.with(|left| right.with(|right| compare(left, right)))?,
            error_handler,
        )
        .map(Memo::into_rx)
}

pub trait ReactivePartialEq: RxRead + Clone {
    fn equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope;

    fn not_equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope;
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone,
{
    fn equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left == right,
            error_handler,
        )
    }

    fn not_equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
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
    fn greater_than<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn less_than<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn greater_than_or_equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn less_than_or_equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone,
{
    fn greater_than<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left > right,
            error_handler,
        )
    }

    fn less_than<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left < right,
            error_handler,
        )
    }

    fn greater_than_or_equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        compare(
            scope,
            self.clone(),
            other,
            |left, right| left >= right,
            error_handler,
        )
    }

    fn less_than_or_equals<'scope, O>(
        &self,
        scope: Scope<'scope>,
        other: O,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        O: ReactiveSource<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
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
