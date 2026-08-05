use crate::{Rx, RxValueKind, reactivity::ReactiveSource};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

#[doc(hidden)]
pub mod ops_impl {
    use std::ops::*;

    macro_rules! binary {
        ($($name:ident: $trait:ident),* $(,)?) => {
            $(
                pub fn $name<T>(left: &T, right: &T) -> T
                where
                    for<'a> &'a T: $trait<&'a T, Output = T>,
                {
                    left.$name(right)
                }
            )*
        };
    }

    binary!(
        add: Add,
        sub: Sub,
        mul: Mul,
        div: Div,
        rem: Rem,
        bitand: BitAnd,
        bitor: BitOr,
        bitxor: BitXor,
        shl: Shl,
        shr: Shr,
    );

    pub fn neg<T>(value: &T) -> T
    where
        for<'a> &'a T: Neg<Output = T>,
    {
        value.neg()
    }

    pub fn not<T>(value: &T) -> T
    where
        for<'a> &'a T: Not<Output = T>,
    {
        value.not()
    }
}

fn binary_op<'scope, T, R>(left: Rx<'scope, T>, right: R, op: fn(&T, &T) -> T) -> Rx<'scope, T>
where
    T: 'scope,
    R: ReactiveSource<'scope, Value = T>,
{
    let scope = left.scope();
    let right = right.into_promotion_plan();
    let mut inputs = left.runtime_inputs();
    inputs.extend(&right.inputs());
    scope.assert_inputs(&inputs);
    let right = right.materialize_unchecked(scope);
    scope.derived_from(inputs, move || {
        left.with(|left| right.with(|right| op(left, right)))
    })
}

fn unary_op<'scope, T>(value: Rx<'scope, T>, op: fn(&T) -> T) -> Rx<'scope, T>
where
    T: 'scope,
{
    let scope = value.scope();
    scope.derived_from(value.runtime_inputs(), move || value.with(op))
}

macro_rules! impl_rx_binary {
    ($trait:ident, $method:ident, $op:ident) => {
        impl<'scope, T, R> $trait<R> for Rx<'scope, T, RxValueKind>
        where
            T: Clone + 'scope,
            for<'a> &'a T: $trait<&'a T, Output = T>,
            R: ReactiveSource<'scope, Value = T>,
        {
            type Output = Rx<'scope, T>;

            fn $method(self, right: R) -> Self::Output {
                binary_op(self, right, ops_impl::$op::<T>)
            }
        }
    };
}

macro_rules! impl_rx_unary {
    ($trait:ident, $method:ident, $op:ident) => {
        impl<'scope, T> $trait for Rx<'scope, T, RxValueKind>
        where
            T: Clone + 'scope,
            for<'a> &'a T: $trait<Output = T>,
        {
            type Output = Rx<'scope, T>;

            fn $method(self) -> Self::Output {
                unary_op(self, ops_impl::$op::<T>)
            }
        }
    };
}

impl_rx_binary!(Add, add, add);
impl_rx_binary!(Sub, sub, sub);
impl_rx_binary!(Mul, mul, mul);
impl_rx_binary!(Div, div, div);
impl_rx_binary!(Rem, rem, rem);
impl_rx_binary!(BitAnd, bitand, bitand);
impl_rx_binary!(BitOr, bitor, bitor);
impl_rx_binary!(BitXor, bitxor, bitxor);
impl_rx_binary!(Shl, shl, shl);
impl_rx_binary!(Shr, shr, shr);
impl_rx_unary!(Neg, neg, neg);
impl_rx_unary!(Not, not, not);
