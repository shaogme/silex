use crate::{ErrorHandlerInput, Rx, SilexResult, reactivity::ReactiveSource, traits::RxRead};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

macro_rules! binary_method {
    ($name:ident, $trait:ident, $op:ident) => {
        pub fn $name<R, H>(self, right: R, error_handler: H) -> SilexResult<Rx<'scope, T>>
        where
            T: PartialEq + 'scope,
            for<'a> &'a T: $trait<&'a T, Output = T>,
            R: ReactiveSource<'scope, Value = T>,
            H: ErrorHandlerInput<'scope>,
        {
            binary_op(self, right, ops_impl::$op::<T>, error_handler)
        }
    };
}

macro_rules! unary_method {
    ($name:ident, $trait:ident, $op:ident) => {
        pub fn $name<H>(self, error_handler: H) -> SilexResult<Rx<'scope, T>>
        where
            T: PartialEq + 'scope,
            for<'a> &'a T: $trait<Output = T>,
            H: ErrorHandlerInput<'scope>,
        {
            unary_op(self, ops_impl::$op::<T>, error_handler)
        }
    };
}

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

fn binary_op<'scope, T, R, H>(
    left: Rx<'scope, T>,
    right: R,
    op: fn(&T, &T) -> T,
    error_handler: H,
) -> SilexResult<Rx<'scope, T>>
where
    T: PartialEq + 'scope,
    R: ReactiveSource<'scope, Value = T>,
    H: ErrorHandlerInput<'scope>,
{
    let error_handler = error_handler.handler_ref();
    let owner = left.owner();
    let right = right.into_promotion_plan();
    let right = right.materialize(owner, error_handler)?;
    owner
        .computed(
            move || left.with(|left| right.with(|right| op(left, right)))?,
            error_handler,
        )
        .map(crate::Computed::into_rx)
}

fn unary_op<'scope, T, H>(
    value: Rx<'scope, T>,
    op: fn(&T) -> T,
    error_handler: H,
) -> SilexResult<Rx<'scope, T>>
where
    T: PartialEq + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    let error_handler = error_handler.handler_ref();
    let owner = value.owner();
    owner
        .computed(move || value.with(op), error_handler)
        .map(crate::Computed::into_rx)
}

impl<'scope, T> Rx<'scope, T> {
    binary_method!(add, Add, add);
    binary_method!(sub, Sub, sub);
    binary_method!(mul, Mul, mul);
    binary_method!(div, Div, div);
    binary_method!(rem, Rem, rem);
    binary_method!(bitand, BitAnd, bitand);
    binary_method!(bitor, BitOr, bitor);
    binary_method!(bitxor, BitXor, bitxor);
    binary_method!(shl, Shl, shl);
    binary_method!(shr, Shr, shr);

    unary_method!(neg, Neg, neg);
    unary_method!(not, Not, not);
}
