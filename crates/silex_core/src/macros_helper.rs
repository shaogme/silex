use crate::{
    ErrorReporter, OwnerAccess, Rx, SilexResult, reactivity::ReactiveSource, traits::RxRead,
};

#[derive(Clone, Copy)]
pub struct ComputedContext<'scope> {
    pub owner: OwnerAccess<'scope>,
    pub error_handler: ErrorReporter<'scope>,
}

impl<'scope> ComputedContext<'scope> {
    pub fn new(owner: OwnerAccess<'scope>, error_handler: ErrorReporter<'scope>) -> Self {
        Self {
            owner,
            error_handler,
        }
    }
}

#[inline]
pub fn map1_static<'scope, S, U>(
    ctx: ComputedContext<'scope>,
    source: S,
    f: fn(&S::Value) -> U,
) -> SilexResult<Rx<'scope, U>>
where
    S: RxRead + ReactiveSource<'scope> + 'scope,
    S::Value: Sized + 'scope,
    U: 'scope,
{
    let owner = ctx.owner;
    let error_handler = ctx.error_handler;
    let source = source.into_promotion_plan();
    let source = source.materialize(owner, error_handler)?;
    owner
        .computed_always(move || source.with(f), error_handler)
        .map(crate::Computed::into_rx)
}

#[inline]
pub fn map2_static<'scope, A, B, U>(
    ctx: ComputedContext<'scope>,
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
    let owner = ctx.owner;
    let error_handler = ctx.error_handler;
    let left = left.into_promotion_plan();
    let right = right.into_promotion_plan();
    let left = left.materialize(owner, error_handler)?;
    let right = right.materialize(owner, error_handler)?;
    owner
        .computed_always(
            move || left.with(|left| right.with(|right| f(left, right)))?,
            error_handler,
        )
        .map(crate::Computed::into_rx)
}

#[inline]
pub fn map3_static<'scope, A, B, C, U>(
    ctx: ComputedContext<'scope>,
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
    let owner = ctx.owner;
    let error_handler = ctx.error_handler;
    let first = first.into_promotion_plan();
    let second = second.into_promotion_plan();
    let third = third.into_promotion_plan();
    let first = first.materialize(owner, error_handler)?;
    let second = second.materialize(owner, error_handler)?;
    let third = third.materialize(owner, error_handler)?;
    owner
        .computed_always(
            move || {
                first.with(|first| {
                    second.with(|second| third.with(|third| f(first, second, third)))?
                })?
            },
            error_handler,
        )
        .map(crate::Computed::into_rx)
}
