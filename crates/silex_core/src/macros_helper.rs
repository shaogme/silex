use crate::{
    Rx, Scope,
    traits::{IntoRx, RxRead},
};

#[inline]
pub fn map1_static<'scope, S, U>(
    scope: &Scope<'scope>,
    source: S,
    f: fn(&S::Value) -> U,
) -> Rx<'scope, U>
where
    S: RxRead + IntoRx<'scope> + 'scope,
    S::Value: Sized + 'scope,
    U: 'scope,
{
    source.into_rx(scope).map(f)
}

#[inline]
pub fn map2_static<'scope, A, B, U>(
    scope: &Scope<'scope>,
    left: A,
    right: B,
    f: fn(&A::Value, &B::Value) -> U,
) -> Rx<'scope, U>
where
    A: RxRead + IntoRx<'scope> + 'scope,
    B: RxRead + IntoRx<'scope> + 'scope,
    A::Value: Sized + 'scope,
    B::Value: Sized + 'scope,
    U: 'scope,
{
    let left = left.into_rx(scope);
    let right = right.into_rx(scope);
    let scope = *scope;
    scope.derived(move || left.with(|left| right.with(|right| f(left, right))))
}

#[inline]
pub fn map3_static<'scope, A, B, C, U>(
    scope: &Scope<'scope>,
    first: A,
    second: B,
    third: C,
    f: fn(&A::Value, &B::Value, &C::Value) -> U,
) -> Rx<'scope, U>
where
    A: RxRead + IntoRx<'scope> + 'scope,
    B: RxRead + IntoRx<'scope> + 'scope,
    C: RxRead + IntoRx<'scope> + 'scope,
    A::Value: Sized + 'scope,
    B::Value: Sized + 'scope,
    C::Value: Sized + 'scope,
    U: 'scope,
{
    let first = first.into_rx(scope);
    let second = second.into_rx(scope);
    let third = third.into_rx(scope);
    let scope = *scope;
    scope.derived(move || {
        first.with(|first| second.with(|second| third.with(|third| f(first, second, third))))
    })
}
