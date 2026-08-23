use crate::traits::{
    RxOptionGuard, RxReadLease, RxRefGuard, RxTupleGuard1, RxTupleGuard2, RxTupleGuard3,
    RxTupleGuard4, RxTupleGuard5, RxTupleGuard6,
};
use crate::{SilexError, SilexResult};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// A runtime-backed read guard with `silex_core` error conversion.
pub struct ReadGuard<'scope, T: ?Sized> {
    inner: silex_reactivity::ReadGuard<'scope, T>,
}

impl<'scope, T: ?Sized> ReadGuard<'scope, T> {
    pub(crate) fn new(inner: silex_reactivity::ReadGuard<'scope, T>) -> Self {
        Self { inner }
    }

    pub fn finish(self) -> SilexResult<()> {
        self.inner.finish().map_err(SilexError::fatal)
    }
}

impl<T: ?Sized> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized> RxReadLease for ReadGuard<'_, T> {
    fn finish(self) -> SilexResult<()> {
        ReadGuard::finish(self)
    }
}

impl<T: ?Sized> RxRefGuard<T> for ReadGuard<'_, T> {
    fn with_ref<U>(&self, f: impl for<'view> FnOnce(&'view T) -> U) -> U {
        f(&self.inner)
    }
}

/// A runtime-backed write guard with `silex_core` error conversion.
pub struct WriteGuard<'scope, T: ?Sized> {
    inner: silex_reactivity::WriteGuard<'scope, T>,
}

impl<'scope, T: ?Sized> WriteGuard<'scope, T> {
    pub(crate) fn new(inner: silex_reactivity::WriteGuard<'scope, T>) -> Self {
        Self { inner }
    }

    pub fn commit(self) -> SilexResult<()> {
        self.inner.commit().map_err(SilexError::fatal)
    }

    pub fn abort(self) -> SilexResult<()> {
        self.inner.abort().map_err(SilexError::fatal)
    }
}

impl<T: ?Sized> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A read guard backed by a borrow of a non-runtime value.
pub struct BorrowedReadGuard<'a, T: ?Sized> {
    value: &'a T,
}

impl<'a, T: ?Sized> BorrowedReadGuard<'a, T> {
    pub(crate) fn new(value: &'a T) -> Self {
        Self { value }
    }
}

impl<T: ?Sized> Deref for BorrowedReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T: ?Sized> RxReadLease for BorrowedReadGuard<'_, T> {
    fn finish(self) -> SilexResult<()> {
        Ok(())
    }
}

impl<T: ?Sized> RxRefGuard<T> for BorrowedReadGuard<'_, T> {
    fn with_ref<U>(&self, f: impl for<'view> FnOnce(&'view T) -> U) -> U {
        f(self.value)
    }
}

/// A read guard that retains its source guard while exposing a safe projection.
pub struct MappedReadGuard<G, F, S: ?Sized, O: ?Sized> {
    source: G,
    getter: F,
    marker: PhantomData<fn(&S) -> &O>,
}

impl<G, F, S: ?Sized, O: ?Sized> MappedReadGuard<G, F, S, O> {
    pub(crate) fn new(source: G, getter: F) -> Self {
        Self {
            source,
            getter,
            marker: PhantomData,
        }
    }
}

impl<G, F, S: ?Sized, O: ?Sized> Deref for MappedReadGuard<G, F, S, O>
where
    G: Deref<Target = S>,
    F: for<'a> Fn(&'a S) -> &'a O,
{
    type Target = O;

    fn deref(&self) -> &Self::Target {
        (self.getter)(&self.source)
    }
}

impl<G, F, S: ?Sized, O: ?Sized> RxReadLease for MappedReadGuard<G, F, S, O>
where
    G: RxReadLease,
{
    fn finish(self) -> SilexResult<()> {
        self.source.finish()
    }
}

impl<G, F, S: ?Sized, O: ?Sized> RxRefGuard<O> for MappedReadGuard<G, F, S, O>
where
    G: RxRefGuard<S>,
    F: for<'a> Fn(&'a S) -> &'a O,
{
    fn with_ref<U>(&self, f: impl for<'view> FnOnce(&'view O) -> U) -> U {
        self.source.with_ref(|source| f((self.getter)(source)))
    }
}

/// A read guard that projects a runtime payload to an optional borrowed view.
pub struct MappedOptionReadGuard<G, F, S: ?Sized, T> {
    source: G,
    getter: F,
    marker: PhantomData<fn(&S) -> &T>,
}

impl<G, F, S: ?Sized, T> MappedOptionReadGuard<G, F, S, T> {
    pub(crate) fn new(source: G, getter: F) -> Self {
        Self {
            source,
            getter,
            marker: PhantomData,
        }
    }
}

impl<G, F, S: ?Sized, T> RxReadLease for MappedOptionReadGuard<G, F, S, T>
where
    G: RxReadLease,
{
    fn finish(self) -> SilexResult<()> {
        self.source.finish()
    }
}

impl<G, F, S: ?Sized, T> RxOptionGuard<T> for MappedOptionReadGuard<G, F, S, T>
where
    G: RxRefGuard<S>,
    F: for<'a> Fn(&'a S) -> Option<&'a T>,
{
    fn with_option<U>(&self, f: impl for<'view> FnOnce(Option<&'view T>) -> U) -> U {
        self.source.with_ref(|source| f((self.getter)(source)))
    }
}

/// A guard used by [`crate::Rx`] to erase its concrete read-source variant.
pub enum RxReadGuard<'scope, T> {
    ReadSignal(ReadGuard<'scope, T>),
    Computed(ReadGuard<'scope, T>),
    Stored(ReadGuard<'scope, T>),
}

impl<T> RxReadGuard<'_, T> {
    pub fn finish(self) -> SilexResult<()> {
        match self {
            Self::ReadSignal(guard) | Self::Computed(guard) | Self::Stored(guard) => guard.finish(),
        }
    }
}

impl<T> RxReadLease for RxReadGuard<'_, T> {
    fn finish(self) -> SilexResult<()> {
        RxReadGuard::finish(self)
    }
}

impl<T> RxRefGuard<T> for RxReadGuard<'_, T> {
    fn with_ref<U>(&self, f: impl for<'view> FnOnce(&'view T) -> U) -> U {
        match self {
            Self::ReadSignal(guard) => guard.with_ref(f),
            Self::Computed(guard) => guard.with_ref(f),
            Self::Stored(guard) => guard.with_ref(f),
        }
    }
}

impl<T> Deref for RxReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::ReadSignal(guard) | Self::Computed(guard) | Self::Stored(guard) => guard,
        }
    }
}

macro_rules! define_tuple_read_guard {
    ($guard:ident, $tuple_guard:ident, $($index:tt => $type:ident : $var:ident),+) => {
        pub struct $guard<$($type),+>($($type),+);

        impl<$($type),+> $guard<$($type),+> {
            pub(crate) fn new($($var: $type),+) -> Self {
                Self($($var),+)
            }
        }

        impl<$($type),+> RxReadLease for $guard<$($type),+>
        where
            $($type: RxReadLease,)+
        {
            fn finish(self) -> SilexResult<()> {
                let Self($($var),+) = self;
                let mut first_error = None;
                $(
                    if let Err(error) = $var.finish() {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                )+
                match first_error {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            }
        }
    };
}

define_tuple_read_guard!(TupleReadGuard1, RxTupleGuard1, 0 => A : a);
define_tuple_read_guard!(TupleReadGuard2, RxTupleGuard2, 0 => A : a, 1 => B : b);
define_tuple_read_guard!(TupleReadGuard3, RxTupleGuard3, 0 => A : a, 1 => B : b, 2 => C : c);
define_tuple_read_guard!(TupleReadGuard4, RxTupleGuard4, 0 => A : a, 1 => B : b, 2 => C : c, 3 => D : d);
define_tuple_read_guard!(
    TupleReadGuard5,
    RxTupleGuard5,
    0 => A : a,
    1 => B : b,
    2 => C : c,
    3 => D : d,
    4 => E : e
);
define_tuple_read_guard!(
    TupleReadGuard6,
    RxTupleGuard6,
    0 => A : a,
    1 => B : b,
    2 => C : c,
    3 => D : d,
    4 => E : e,
    5 => F : f
);

macro_rules! tuple_with_refs {
    ($guard:expr, $callback:ident; $($index:tt => $binding:ident),+) => {
        tuple_with_refs!(@step $guard, $callback, (); $($index => $binding),+)
    };
    (@step $guard:expr, $callback:ident, ($($binding:ident,)*); $index:tt => $next:ident $(, $rest_index:tt => $rest:ident)*) => {
        $guard.$index.with_ref(|$next| {
            tuple_with_refs!(@step $guard, $callback, ($($binding,)* $next,); $($rest_index => $rest),*)
        })
    };
    (@step $guard:expr, $callback:ident, ($($binding:ident,)*);) => {
        $callback(($($binding,)*))
    };
}

macro_rules! impl_tuple_ref_guard {
    ($guard:ident, $tuple_guard:ident, $($index:tt => $binding:ident : $field:ident : $type:ident),+) => {
        impl<$($field, $type),+> $tuple_guard<$($type),+> for $guard<$($field),+>
        where
            $($field: RxRefGuard<$type>,)+
        {
            fn with_tuple<U>(
                &self,
                f: impl for<'view> FnOnce(($(&'view $type,)+)) -> U,
            ) -> U {
                tuple_with_refs!(self, f; $($index => $binding),+)
            }
        }
    };
}

impl_tuple_ref_guard!(TupleReadGuard1, RxTupleGuard1, 0 => first : A : TA);
impl_tuple_ref_guard!(TupleReadGuard2, RxTupleGuard2, 0 => first : A : TA, 1 => second : B : TB);
impl_tuple_ref_guard!(TupleReadGuard3, RxTupleGuard3, 0 => first : A : TA, 1 => second : B : TB, 2 => third : C : TC);
impl_tuple_ref_guard!(
    TupleReadGuard4,
    RxTupleGuard4,
    0 => first : A : TA,
    1 => second : B : TB,
    2 => third : C : TC,
    3 => fourth : D : TD
);
impl_tuple_ref_guard!(
    TupleReadGuard5,
    RxTupleGuard5,
    0 => first : A : TA,
    1 => second : B : TB,
    2 => third : C : TC,
    3 => fourth : D : TD,
    4 => fifth : E : TE
);
impl_tuple_ref_guard!(
    TupleReadGuard6,
    RxTupleGuard6,
    0 => first : A : TA,
    1 => second : B : TB,
    2 => third : C : TC,
    3 => fourth : D : TD,
    4 => fifth : E : TE,
    5 => sixth : F : TF
);
