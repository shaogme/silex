use crate::{ErrorReporter, Rx, Scope, SilexResult, reactivity::ReactiveSource, traits::RxRead};

#[derive(Clone, Copy)]
pub struct DerivedContext<'scope> {
    pub scope: Scope<'scope>,
    pub error_handler: ErrorReporter<'scope>,
}

impl<'scope> DerivedContext<'scope> {
    pub fn new(scope: Scope<'scope>, error_handler: ErrorReporter<'scope>) -> Self {
        Self {
            scope,
            error_handler,
        }
    }
}

#[inline]
pub fn map1_static<'scope, S, U>(
    context: DerivedContext<'scope>,
    source: S,
    f: fn(&S::Value) -> U,
) -> SilexResult<Rx<'scope, U>>
where
    S: RxRead + ReactiveSource<'scope> + 'scope,
    S::Value: Sized + 'scope,
    U: 'scope,
{
    let scope = context.scope;
    let error_handler = context.error_handler;
    let source = source.into_promotion_plan();
    let source = source.materialize(scope, error_handler)?;
    scope.derived(move || source.with(f), error_handler)
}

#[inline]
pub fn map2_static<'scope, A, B, U>(
    context: DerivedContext<'scope>,
    left: A,
    right: B,
    f: fn(&A::Value, &B::Value) -> U,
) -> SilexResult<Rx<'scope, U>>
where
    A: RxRead + ReactiveSource<'scope> + 'scope,
    B: RxRead + ReactiveSource<'scope> + 'scope,
    A::Value: Sized + 'scope,
    B::Value: Sized + 'scope,
    U: 'scope,
{
    let scope = context.scope;
    let error_handler = context.error_handler;
    let left = left.into_promotion_plan();
    let right = right.into_promotion_plan();
    let left = left.materialize(scope, error_handler)?;
    let right = right.materialize(scope, error_handler)?;
    scope.derived(
        move || left.with(|left| right.with(|right| f(left, right)))?,
        error_handler,
    )
}

#[inline]
pub fn map3_static<'scope, A, B, C, U>(
    context: DerivedContext<'scope>,
    first: A,
    second: B,
    third: C,
    f: fn(&A::Value, &B::Value, &C::Value) -> U,
) -> SilexResult<Rx<'scope, U>>
where
    A: RxRead + ReactiveSource<'scope> + 'scope,
    B: RxRead + ReactiveSource<'scope> + 'scope,
    C: RxRead + ReactiveSource<'scope> + 'scope,
    A::Value: Sized + 'scope,
    B::Value: Sized + 'scope,
    C::Value: Sized + 'scope,
    U: 'scope,
{
    let scope = context.scope;
    let error_handler = context.error_handler;
    let first = first.into_promotion_plan();
    let second = second.into_promotion_plan();
    let third = third.into_promotion_plan();
    let first = first.materialize(scope, error_handler)?;
    let second = second.materialize(scope, error_handler)?;
    let third = third.materialize(scope, error_handler)?;
    scope.derived(
        move || {
            first.with(|first| {
                second.with(|second| third.with(|third| f(first, second, third)))?
            })?
        },
        error_handler,
    )
}
