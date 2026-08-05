use crate::{Rx, Scope, reactivity::ReactiveSource, traits::RxRead};

#[inline]
pub fn map1_static<'scope, S, U>(
    scope: Scope<'scope>,
    source: S,
    f: fn(&S::Value) -> U,
) -> Rx<'scope, U>
where
    S: RxRead + ReactiveSource<'scope> + 'scope,
    S::Value: Sized + 'scope,
    U: 'scope,
{
    let source = source.into_promotion_plan();
    let inputs = source.inputs();
    scope.assert_inputs(&inputs);
    let source = source.materialize_unchecked(scope);
    scope.derived_from(inputs, move || source.with(f))
}

#[inline]
pub fn map2_static<'scope, A, B, U>(
    scope: Scope<'scope>,
    left: A,
    right: B,
    f: fn(&A::Value, &B::Value) -> U,
) -> Rx<'scope, U>
where
    A: RxRead + ReactiveSource<'scope> + 'scope,
    B: RxRead + ReactiveSource<'scope> + 'scope,
    A::Value: Sized + 'scope,
    B::Value: Sized + 'scope,
    U: 'scope,
{
    let left = left.into_promotion_plan();
    let right = right.into_promotion_plan();
    let mut inputs = left.inputs();
    inputs.extend(&right.inputs());
    scope.assert_inputs(&inputs);
    let left = left.materialize_unchecked(scope);
    let right = right.materialize_unchecked(scope);
    scope.derived_from(inputs, move || {
        left.with(|left| right.with(|right| f(left, right)))
    })
}

#[inline]
pub fn map3_static<'scope, A, B, C, U>(
    scope: Scope<'scope>,
    first: A,
    second: B,
    third: C,
    f: fn(&A::Value, &B::Value, &C::Value) -> U,
) -> Rx<'scope, U>
where
    A: RxRead + ReactiveSource<'scope> + 'scope,
    B: RxRead + ReactiveSource<'scope> + 'scope,
    C: RxRead + ReactiveSource<'scope> + 'scope,
    A::Value: Sized + 'scope,
    B::Value: Sized + 'scope,
    C::Value: Sized + 'scope,
    U: 'scope,
{
    let first = first.into_promotion_plan();
    let second = second.into_promotion_plan();
    let third = third.into_promotion_plan();
    let mut inputs = first.inputs();
    inputs.extend(&second.inputs());
    inputs.extend(&third.inputs());
    scope.assert_inputs(&inputs);
    let first = first.materialize_unchecked(scope);
    let second = second.materialize_unchecked(scope);
    let third = third.materialize_unchecked(scope);
    scope.derived_from(inputs, move || {
        first.with(|first| second.with(|second| third.with(|third| f(first, second, third))))
    })
}
