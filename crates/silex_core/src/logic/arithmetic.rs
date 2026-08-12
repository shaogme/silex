use crate::{ErrorReporter, Rx, RxValueKind, SilexResult, reactivity::ReactiveSource};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

macro_rules! binary_method {
    ($name:ident, $trait:ident, $op:ident) => {
        pub fn $name<R>(
            self,
            right: R,
            error_handler: ErrorReporter<'scope>,
        ) -> SilexResult<Rx<'scope, T>>
        where
            T: 'scope,
            for<'a> &'a T: $trait<&'a T, Output = T>,
            R: ReactiveSource<'scope, Value = T>,
        {
            binary_op(self, right, ops_impl::$op::<T>, error_handler)
        }
    };
}

macro_rules! unary_method {
    ($name:ident, $trait:ident, $op:ident) => {
        pub fn $name(self, error_handler: ErrorReporter<'scope>) -> SilexResult<Rx<'scope, T>>
        where
            T: 'scope,
            for<'a> &'a T: $trait<Output = T>,
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

fn binary_op<'scope, T, R>(
    left: Rx<'scope, T>,
    right: R,
    op: fn(&T, &T) -> T,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<Rx<'scope, T>>
where
    T: 'scope,
    R: ReactiveSource<'scope, Value = T>,
{
    let scope = left.scope();
    let right = right.into_promotion_plan();
    let mut inputs = left.runtime_inputs();
    inputs.extend(&right.inputs());
    scope.validate_inputs(&inputs)?;
    let right = right.materialize(scope, error_handler)?;
    scope.derived_from(
        inputs,
        move || left.with(|left| right.with(|right| op(left, right)))?,
        error_handler,
    )
}

fn unary_op<'scope, T>(
    value: Rx<'scope, T>,
    op: fn(&T) -> T,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<Rx<'scope, T>>
where
    T: 'scope,
{
    let scope = value.scope();
    scope.derived_from(
        value.runtime_inputs(),
        move || value.with(op),
        error_handler,
    )
}

impl<'scope, T> Rx<'scope, T, RxValueKind> {
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
