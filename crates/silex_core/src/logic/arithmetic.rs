use crate::{Rx, RxValueKind, traits::IntoRx};
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

fn binary_op<'scope, 'run, T, R>(
    left: Rx<'scope, 'run, T>,
    right: R,
    op: fn(&T, &T) -> T,
) -> Rx<'scope, 'run, T>
where
    T: 'scope,
    R: IntoRx<'scope, 'run, Value = T>,
{
    let scope = left.scope();
    let right = right.into_rx(&scope);
    scope.derived(move || left.with(|left| right.with(|right| op(left, right))))
}

fn unary_op<'scope, 'run, T>(value: Rx<'scope, 'run, T>, op: fn(&T) -> T) -> Rx<'scope, 'run, T>
where
    T: 'scope,
{
    let scope = value.scope();
    scope.derived(move || value.with(op))
}

macro_rules! impl_rx_binary {
    ($trait:ident, $method:ident, $op:ident) => {
        impl<'scope, 'run, T, R> $trait<R> for Rx<'scope, 'run, T, RxValueKind>
        where
            T: Clone + 'run,
            for<'a> &'a T: $trait<&'a T, Output = T>,
            R: IntoRx<'scope, 'run, Value = T>,
        {
            type Output = Rx<'scope, 'run, T>;

            fn $method(self, right: R) -> Self::Output {
                binary_op(self, right, ops_impl::$op::<T>)
            }
        }
    };
}

macro_rules! impl_rx_unary {
    ($trait:ident, $method:ident, $op:ident) => {
        impl<'scope, 'run, T> $trait for Rx<'scope, 'run, T, RxValueKind>
        where
            T: Clone + 'run,
            for<'a> &'a T: $trait<Output = T>,
        {
            type Output = Rx<'scope, 'run, T>;

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
