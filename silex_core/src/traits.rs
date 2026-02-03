//! A series of traits to implement the behavior of reactive primitive, especially signals.
//!
//! ## Design Philosophy: Zero-Copy First
//!
//! The core insight of this trait system is that [`With`] (closure-based access) is the fundamental
//! primitive for accessing reactive values. This is because:
//!
//! 1. **Memory Layout**: Signal values are stored in an arena (Map/Vec). To access them, we need to
//!    hold a lock on the storage and provide a reference.
//! 2. **Zero-Copy**: Using closures allows us to work with `&T` directly without cloning.
//! 3. **?Sized Support**: Closures can work with dynamically-sized types like `str` or `[T]`.
//!
//! The [`Get`] trait (which clones the value) is deliberately NOT the core primitive. It's only
//! available as a convenience method when `T: Clone + Sized`, and should be avoided on hot paths
//! where cloning is expensive.
//!
//! ## Important: Tuples Are NOT Signals
//!
//! Tuples of signals `(Signal<A>, Signal<B>)` cannot implement zero-copy access for `&(A, B)`
//! because A and B are stored in different memory locations. Instead of silently cloning,
//! we provide the [`batch_read!`] macro for explicit zero-copy multi-signal access.
//!
//! ## Principles
//! 1. **Composition**: Most of the traits are implemented as combinations of more primitive base traits,
//!    and blanket implemented for all types that implement those traits.
//! 2. **Fallibility**: Most traits includes a `try_` variant, which returns `None` if the method
//!    fails (e.g., if signals are arena allocated and this can't be found).
//! 3. **Zero-Copy**: Prefer [`With`]/[`WithUntracked`] over [`Get`]/[`GetUntracked`] to avoid cloning.
//!
//! ## Metadata Traits
//! - [`DefinedAt`] is used for debugging in the case of errors and should be implemented for all
//!   signal types.
//! - [`IsDisposed`] checks whether a signal is currently accessible.
//!
//! ## Base Traits (Core - Implement These)
//! | Trait               | Mode    | Description                                                                           |
//! |---------------------|---------|---------------------------------------------------------------------------------------|
//! | [`Track`]           | —       | Tracks changes to this value, adding it as a source of the current reactive observer. |
//! | [`Notify`]          | —       | Notifies subscribers that this value has changed.                                     |
//! | [`WithUntracked`]   | Closure | **Core**: Gives immutable access to the value of this signal without tracking.        |
//! | [`UpdateUntracked`] | Closure | Gives mutable access to update the value of this signal without notifying.            |
//!
//! ## Derived Traits (Blanket Implemented)
//!
//! ### Access
//! | Trait             | Mode          | Composition                        | Description
//! |-------------------|---------------|------------------------------------|------------
//! | [`With`]          | `fn(&T) -> U` | [`WithUntracked`] + [`Track`]      | **Core**: Applies closure to the current value with reactive tracking.
//! | [`GetUntracked`]  | `T`           | [`WithUntracked`] + [`Clone`]      | Extension: Clones the current value (requires `T: Clone + Sized`).
//! | [`Get`]           | `T`           | [`With`] + [`Clone`]               | Extension: Clones with reactive tracking (requires `T: Clone + Sized`).
//! | [`Map`]           | `Derived<S,F>`| [`With`]                           | Creates a derived signal from this signal.
//!
//! ### Update
//! | Trait               | Mode          | Composition                        | Description
//! |---------------------|---------------|------------------------------------|------------
//! | [`Update`]          | `fn(&mut T)`  | [`UpdateUntracked`] + [`Notify`]   | Applies closure to the current value to update it, and notifies subscribers.
//! | [`Set`]             | `T`           | [`Update`]                         | Sets the value to a new value, and notifies subscribers.
//! | [`SignalSetter`]    | `Fn`          | [`Set`]                            | Creates a closure that sets the signal to a specific value.
//! | [`SignalUpdater`]   | `Fn`          | [`Update`]                         | Creates a closure that updates the signal using a specific function.
//!
//! ## Using the Traits
//!
//! These traits are designed so that you can implement as few as possible, and the rest will be
//! implemented automatically.
//!
//! For example, if you have a struct for which you can implement [`WithUntracked`] and [`Track`], then
//! [`With`] will be implemented automatically (as will [`GetUntracked`] and
//! [`Get`] for `Clone + Sized` types).
//!
//! ## Multi-Signal Access
//!
//! For accessing multiple signals without cloning, use the [`batch_read!`] macro:
//!
//! ```rust,ignore
//! let (name_signal, age_signal) = (signal("Alice".to_string()), signal(42));
//!
//! // Zero-copy multi-signal access:
//! batch_read!(name_signal, age_signal => |name: &String, age: &i32| {
//!     println!("{} is {} years old", name, age);
//! });
//! ```

/// A module containing static helper functions for reactive operations.
/// usage of these avoids generating unique closures for every operator implementation.
#[doc(hidden)]
pub mod ops_impl {
    use std::ops::*;

    #[inline]
    pub fn add<T: Add<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().add(rhs.clone())
    }
    #[inline]
    pub fn sub<T: Sub<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().sub(rhs.clone())
    }
    #[inline]
    pub fn mul<T: Mul<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().mul(rhs.clone())
    }
    #[inline]
    pub fn div<T: Div<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().div(rhs.clone())
    }
    #[inline]
    pub fn rem<T: Rem<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().rem(rhs.clone())
    }
    #[inline]
    pub fn bitand<T: BitAnd<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().bitand(rhs.clone())
    }
    #[inline]
    pub fn bitor<T: BitOr<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().bitor(rhs.clone())
    }
    #[inline]
    pub fn bitxor<T: BitXor<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().bitxor(rhs.clone())
    }
    #[inline]
    pub fn shl<T: Shl<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().shl(rhs.clone())
    }
    #[inline]
    pub fn shr<T: Shr<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
        lhs.clone().shr(rhs.clone())
    }
    #[inline]
    pub fn neg<T: Neg<Output = T> + Clone>(val: &T) -> T {
        val.clone().neg()
    }
    #[inline]
    pub fn not<T: Not<Output = T> + Clone>(val: &T) -> T {
        val.clone().not()
    }
}

///   compiled code size.
///
/// *Note*: This requires the right-hand side operand to implement `IntoSignal`. Primitives (i32, f64, etc.)
/// and Signals already implement this. Custom types used in reactive math operations will need to implement
/// `IntoSignal` manually (mapping to `Constant<T>`).

#[macro_export]
macro_rules! impl_reactive_ops {
    ($target:ident) => {
        $crate::impl_reactive_op!($target, Add, add);
        $crate::impl_reactive_op!($target, Sub, sub);
        $crate::impl_reactive_op!($target, Mul, mul);
        $crate::impl_reactive_op!($target, Div, div);
        $crate::impl_reactive_op!($target, Rem, rem);
        $crate::impl_reactive_op!($target, BitAnd, bitand);
        $crate::impl_reactive_op!($target, BitOr, bitor);
        $crate::impl_reactive_op!($target, BitXor, bitxor);
        $crate::impl_reactive_op!($target, Shl, shl);
        $crate::impl_reactive_op!($target, Shr, shr);

        $crate::impl_reactive_unary_op!($target, Neg, neg);
        $crate::impl_reactive_unary_op!($target, Not, not);
    };
}

#[macro_export]
macro_rules! impl_reactive_op {
    ($target:ident, $trait:ident, $method:ident) => {
        // Op with anything convertible to a Signal (Primitives + Signals)
        impl<T, R> std::ops::$trait<R> for $target<T>
        where
            T: std::ops::$trait<T, Output = T> + Clone + 'static,
            T: PartialEq + 'static,
            R: $crate::traits::IntoSignal<Value = T>,
            R::Signal: 'static,
        {
            type Output =
                $crate::reactivity::ReactiveBinary<$target<T>, R::Signal, fn(&T, &T) -> T>;

            fn $method(self, rhs: R) -> Self::Output {
                $crate::reactivity::ReactiveBinary::new(
                    self,
                    rhs.into_signal(),
                    $crate::traits::ops_impl::$method,
                )
            }
        }
    };
}

#[macro_export]
macro_rules! impl_reactive_unary_op {
    ($target:ident, $trait:ident, $method:ident) => {
        impl<T> std::ops::$trait for $target<T>
        where
            T: std::ops::$trait<Output = T> + Clone + 'static,
            T: PartialEq + 'static,
        {
            type Output = $crate::reactivity::Derived<$target<T>, fn(&T) -> T>;

            fn $method(self) -> Self::Output {
                $crate::reactivity::Derived::new(self, $crate::traits::ops_impl::$method)
            }
        }
    };
}

impl<F, T> DefinedAt for F
where
    F: Fn() -> T,
{
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<F, T> IsDisposed for F
where
    F: Fn() -> T,
{
    fn is_disposed(&self) -> bool {
        false // Closures are never disposed
    }
}

impl<F, T> Track for F
where
    F: Fn() -> T,
{
    fn track(&self) {
        // Closures don't have built-in tracking - tracking happens when
        // the closure accesses signals internally
    }
}

impl<F, T> WithUntracked for F
where
    F: Fn() -> T,
{
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        let val = self();
        Some(fun(&val))
    }
}

macro_rules! impl_tuple_traits {
    ($($T:ident),*) => {
        impl<$($T),*> DefinedAt for ($($T,)*)
        where
            $($T: DefinedAt),*
        {
            fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
                None
            }
        }

        impl<$($T),*> IsDisposed for ($($T,)*)
        where
            $($T: IsDisposed),*
        {
            #[allow(non_snake_case)]
            fn is_disposed(&self) -> bool {
                let ($($T,)*) = self;
                // A tuple is disposed if any of its components is disposed
                $($T.is_disposed() ||)* false
            }
        }

        impl<$($T),*> Track for ($($T,)*)
        where
            $($T: Track),*
        {
            #[allow(non_snake_case)]
            fn track(&self) {
                let ($($T,)*) = self;
                $($T.track();)*
            }
        }

        // NOTE: We intentionally DO NOT implement WithUntracked/With/GetUntracked/Get
        // for tuples. See the comment block above for the rationale.
        // Use `batch_read!` macro instead for zero-copy multi-signal access.
    };
}

impl_tuple_traits!(A, B);
impl_tuple_traits!(A, B, C);
impl_tuple_traits!(A, B, C, D);
impl_tuple_traits!(A, B, C, D, E);
impl_tuple_traits!(A, B, C, D, E, F);

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: With + Clone + 'static
where
    Self::Value: PartialEq + Clone + Sized + 'static,
{
    fn equals<O>(
        &self,
        other: O,
    ) -> ReactiveBinary<Self, O::Signal, fn(&Self::Value, &Self::Value) -> bool>
    where
        O: IntoSignal<Value = Self::Value> + Clone + 'static,
    {
        ReactiveBinary::new(self.clone(), other.into_signal(), |lhs, rhs| lhs == rhs)
    }

    fn not_equals<O>(
        &self,
        other: O,
    ) -> ReactiveBinary<Self, O::Signal, fn(&Self::Value, &Self::Value) -> bool>
    where
        O: IntoSignal<Value = Self::Value> + Clone + 'static,
    {
        ReactiveBinary::new(self.clone(), other.into_signal(), |lhs, rhs| lhs != rhs)
    }
}

impl<S> ReactivePartialEq for S
where
    S: With + Clone + 'static,
    S::Value: PartialEq + Clone + Sized + 'static,
{
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: With + Clone + 'static
where
    Self::Value: PartialOrd + Clone + Sized + 'static,
{
    fn greater_than<O>(
        &self,
        other: O,
    ) -> ReactiveBinary<Self, O::Signal, fn(&Self::Value, &Self::Value) -> bool>
    where
        O: IntoSignal<Value = Self::Value> + Clone + 'static,
    {
        ReactiveBinary::new(self.clone(), other.into_signal(), |lhs, rhs| lhs > rhs)
    }

    fn less_than<O>(
        &self,
        other: O,
    ) -> ReactiveBinary<Self, O::Signal, fn(&Self::Value, &Self::Value) -> bool>
    where
        O: IntoSignal<Value = Self::Value> + Clone + 'static,
    {
        ReactiveBinary::new(self.clone(), other.into_signal(), |lhs, rhs| lhs < rhs)
    }

    fn greater_than_or_equals<O>(
        &self,
        other: O,
    ) -> ReactiveBinary<Self, O::Signal, fn(&Self::Value, &Self::Value) -> bool>
    where
        O: IntoSignal<Value = Self::Value> + Clone + 'static,
    {
        ReactiveBinary::new(self.clone(), other.into_signal(), |lhs, rhs| lhs >= rhs)
    }

    fn less_than_or_equals<O>(
        &self,
        other: O,
    ) -> ReactiveBinary<Self, O::Signal, fn(&Self::Value, &Self::Value) -> bool>
    where
        O: IntoSignal<Value = Self::Value> + Clone + 'static,
    {
        ReactiveBinary::new(self.clone(), other.into_signal(), |lhs, rhs| lhs <= rhs)
    }
}

impl<S> ReactivePartialOrd for S
where
    S: With + Clone + 'static,
    S::Value: PartialOrd + Clone + Sized + 'static,
{
}

// use any_spawner::Executor;
// use futures::{Stream, StreamExt};
use crate::reactivity::{Constant, Derived, Memo, ReactiveBinary, ReadSignal, RwSignal, Signal};

// --- IntoSignal ---

pub trait IntoSignal {
    type Value;
    type Signal: With<Value = Self::Value>;

    fn into_signal(self) -> Self::Signal;
}

macro_rules! impl_into_signal_primitive {
    ($($t:ty),*) => {
        $(
            impl IntoSignal for $t {
                type Value = $t; // Self
                type Signal = Constant<$t>;

                fn into_signal(self) -> Self::Signal {
                    Constant(self)
                }
            }
        )*
    };
}

impl_into_signal_primitive!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

impl IntoSignal for String {
    type Value = String;
    type Signal = Constant<String>;

    fn into_signal(self) -> Self::Signal {
        Constant(self)
    }
}

impl IntoSignal for &str {
    type Value = String;
    type Signal = Constant<String>;

    fn into_signal(self) -> Self::Signal {
        Constant(self.to_string())
    }
}

impl<T: Clone + 'static> IntoSignal for Signal<T> {
    type Value = T;
    type Signal = Signal<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + 'static> IntoSignal for ReadSignal<T> {
    type Value = T;
    type Signal = ReadSignal<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + 'static> IntoSignal for RwSignal<T> {
    type Value = T;
    type Signal = RwSignal<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + PartialEq + 'static> IntoSignal for Memo<T> {
    type Value = T;
    type Signal = Memo<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

impl<T: Clone + 'static> IntoSignal for Constant<T> {
    type Value = T;
    type Signal = Constant<T>;

    fn into_signal(self) -> Self::Signal {
        self
    }
}

// Allows closures to be treated as derived signals automatically.
// E.g. `signal + (|| 5)`
impl<F, T> IntoSignal for F
where
    F: Fn() -> T + 'static,
    T: Clone + 'static,
{
    type Value = T;
    type Signal = Signal<T>;

    fn into_signal(self) -> Self::Signal {
        Signal::derive(self)
    }
}

macro_rules! impl_into_signal_tuple {
    ($($name:ident : $T:ident),+) => {
        impl<$($T),+> IntoSignal for ($($T,)+)
        where
            $($T: IntoSignal),+,
            $($T::Value: Clone + 'static),+,
            $($T::Signal: 'static),+
        {
            type Value = ($($T::Value,)+);
            type Signal = Signal<Self::Value>;

            #[allow(non_snake_case)]
            fn into_signal(self) -> Self::Signal {
                let ($($name,)+) = self;
                $(let $name = $name.into_signal();)+

                Signal::derive(move || {
                    impl_into_signal_tuple_nest!($($name),+)
                })
            }
        }
    }
}

macro_rules! impl_into_signal_tuple_nest {
    ($s1:ident, $s2:ident) => {
        $s1.with(|v1| $s2.with(|v2| (v1.clone(), v2.clone())))
    };
    ($s1:ident, $s2:ident, $s3:ident) => {
        $s1.with(|v1| $s2.with(|v2| $s3.with(|v3| (v1.clone(), v2.clone(), v3.clone()))))
    };
    ($s1:ident, $s2:ident, $s3:ident, $s4:ident) => {
        $s1.with(|v1| {
            $s2.with(|v2| {
                $s3.with(|v3| $s4.with(|v4| (v1.clone(), v2.clone(), v3.clone(), v4.clone())))
            })
        })
    };
}

impl_into_signal_tuple!(idx0: T0, idx1: T1);
impl_into_signal_tuple!(idx0: T0, idx1: T1, idx2: T2);
impl_into_signal_tuple!(idx0: T0, idx1: T1, idx2: T2, idx3: T3);

use std::panic::Location;

#[doc(hidden)]
/// Provides a sensible panic message for accessing disposed signals.
#[macro_export]
macro_rules! unwrap_signal {
    ($signal:ident) => {{
        #[cfg(debug_assertions)]
        let location = std::panic::Location::caller();
        || {
            #[cfg(debug_assertions)]
            {
                panic!(
                    "{}",
                    $crate::traits::panic_getting_disposed_signal(
                        $signal.defined_at(),
                        $signal.debug_name(),
                        location
                    )
                );
            }
            #[cfg(not(debug_assertions))]
            {
                panic!(
                    "Tried to access a reactive value that has already been \
                     disposed."
                );
            }
        }
    }};
}

/// Allows disposing an arena-allocated signal before its owner has been disposed.
pub trait Dispose {
    /// Disposes of the signal. This:
    /// 1. Detaches the signal from the reactive graph, preventing it from triggering
    ///    further updates; and
    /// 2. Drops the value contained in the signal.
    fn dispose(self);
}

/// Allows tracking the value of some reactive data.
pub trait Track {
    /// Subscribes to this signal in the current reactive scope without doing anything with its value.
    #[track_caller]
    fn track(&self);
}

/// Give read-only access to a signal's value by reference inside a closure,
/// without tracking the value reactively.
pub trait WithUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Applies the closure to the value, and returns the result,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    /// Applies the closure to the value, and returns the result.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with_untracked(fun)
            .unwrap_or_else(unwrap_signal!(self))
    }
}

/// Give read-only access to a signal's value by reference inside a closure,
/// and subscribes the active reactive observer (an effect or computed) to changes in its value.
pub trait With: DefinedAt {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Subscribes to the signal, applies the closure to the value, and returns the result,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    /// Subscribes to the signal, applies the closure to the value, and returns the result.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with(fun).unwrap_or_else(unwrap_signal!(self))
    }
}

impl<T> With for T
where
    T: WithUntracked + Track,
{
    type Value = <T as WithUntracked>::Value;

    #[track_caller]
    fn try_with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.track();
        self.try_with_untracked(fun)
    }
}

/// Extension trait: Clones the value of the signal, without tracking.
///
/// This is a **convenience trait** built on top of [`WithUntracked`]. It requires `T: Clone + Sized`.
/// For zero-copy access, prefer using [`WithUntracked::with_untracked`] directly.
///
/// # Performance Note
/// This trait performs a clone operation. On hot paths or with expensive-to-clone types,
/// prefer using [`WithUntracked::with_untracked`] instead.
pub trait GetUntracked: WithUntracked
where
    Self::Value: Clone + Sized,
{
    /// Clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.try_with_untracked(Clone::clone)
    }

    /// Clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked()
            .unwrap_or_else(unwrap_signal!(self))
    }
}

// Blanket implementation: any type with WithUntracked where Value: Clone + Sized gets GetUntracked
impl<T> GetUntracked for T
where
    T: WithUntracked,
    T::Value: Clone + Sized,
{
}

/// Extension trait: Clones the value of the signal, with reactive tracking.
///
/// This is a **convenience trait** built on top of [`With`]. It requires `T: Clone + Sized`.
/// For zero-copy access, prefer using [`With::with`] directly.
///
/// # Performance Note
/// This trait performs a clone operation. On hot paths or with expensive-to-clone types,
/// prefer using [`With::with`] instead.
pub trait Get: With
where
    Self::Value: Clone + Sized,
{
    /// Subscribes to the signal, then clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get(&self) -> Option<Self::Value> {
        self.try_with(Clone::clone)
    }

    /// Subscribes to the signal, then clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(unwrap_signal!(self))
    }
}

// Blanket implementation: any type with With where Value: Clone + Sized gets Get
impl<T> Get for T
where
    T: With,
    T::Value: Clone + Sized,
{
}

/// Allows creating a derived signal from this signal.
///
/// Unlike [`Get`], this trait uses [`WithUntracked`] as its basis, meaning it works
/// with the zero-copy closure-based access pattern.
pub trait Map: Sized {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Creates a derived signal from this signal.
    fn map<U, F>(self, f: F) -> Derived<Self, F>
    where
        F: Fn(&Self::Value) -> U;
}

// Map is based on WithUntracked, not Get - this is intentional for zero-copy support
impl<S> Map for S
where
    S: WithUntracked + Track,
{
    type Value = S::Value;

    fn map<U, F>(self, f: F) -> Derived<Self, F>
    where
        F: Fn(&Self::Value) -> U,
    {
        Derived::new(self, f)
    }
}

/// Allows converting a signal into a memoized signal.
///
/// Requires `Value: Clone + Sized` since memoization needs to clone and store values.
pub trait Memoize: With
where
    Self::Value: Clone + Sized,
{
    /// Creates a memoized signal from this signal.
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: PartialEq + 'static;
}

impl<T> Memoize for T
where
    T: With + Clone + 'static,
    T::Value: Clone + Sized,
{
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: PartialEq + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| this.with(Clone::clone))
    }
}

/// Notifies subscribers of a change in this signal.
pub trait Notify {
    /// Notifies subscribers of a change in this signal.
    #[track_caller]
    fn notify(&self);
}

/// Updates the value of a signal by applying a function that updates it in place,
/// without notifying subscribers.
pub trait UpdateUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value by applying a function, returning the value returned by that function.
    /// Does not notify subscribers that the signal has changed.
    ///
    /// # Panics
    /// Panics if you try to update a signal that has been disposed.
    #[track_caller]
    fn update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> U {
        self.try_update_untracked(fun)
            .unwrap_or_else(unwrap_signal!(self))
    }

    /// Updates the value by applying a function, returning the value returned by that function,
    /// or `None` if the signal has already been disposed.
    /// Does not notify subscribers that the signal has changed.
    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;
}

/// Updates the value of a signal by applying a function that updates it in place,
/// notifying its subscribers that the value has changed.
pub trait Update {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value of the signal and notifies subscribers.
    #[track_caller]
    fn update(&self, fun: impl FnOnce(&mut Self::Value)) {
        self.try_update(fun);
    }

    /// Updates the value of the signal, but only notifies subscribers if the function
    /// returns `true`.
    #[track_caller]
    fn maybe_update(&self, fun: impl FnOnce(&mut Self::Value) -> bool) {
        self.try_maybe_update(|val| {
            let did_update = fun(val);
            (did_update, ())
        });
    }

    /// Updates the value of the signal and notifies subscribers, returning the value that is
    /// returned by the update function, or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        self.try_maybe_update(|val| (true, fun(val)))
    }

    /// Updates the value of the signal, notifying subscribers if the update function returns
    /// `(true, _)`, and returns the value returned by the update function,
    /// or `None` if the signal has already been disposed.
    fn try_maybe_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U>;
}

/// Updates the value of the signal by replacing it.
pub trait Set {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value by replacing it, and notifies subscribers that it has changed.
    fn set(&self, value: Self::Value);

    /// Updates the value by replacing it, and notifies subscribers that it has changed.
    ///
    /// If the signal has already been disposed, returns `Some(value)` with the value that was
    /// passed in. Otherwise, returns `None`.
    fn try_set(&self, value: Self::Value) -> Option<Self::Value>;
}

impl<T> Set for T
where
    T: Update + IsDisposed,
{
    type Value = <Self as Update>::Value;

    #[track_caller]
    fn set(&self, value: Self::Value) {
        self.try_update(|n| *n = value);
    }

    #[track_caller]
    fn try_set(&self, value: Self::Value) -> Option<Self::Value> {
        if self.is_disposed() {
            Some(value)
        } else {
            self.set(value);
            None
        }
    }
}

/// Allows creating a setter closure from this signal.
pub trait SignalSetter: Sized {
    type Value;

    /// Creates a closure that sets the signal to the given value.
    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static;
}

/// Allows creating an updater closure from this signal.
pub trait SignalUpdater: Sized {
    type Value;

    /// Creates a closure that updates the signal using the given function.
    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        F: Fn(&mut Self::Value) + Clone + 'static;
}

/// Checks whether a signal has already been disposed.
pub trait IsDisposed {
    /// If `true`, the signal cannot be accessed without a panic.
    fn is_disposed(&self) -> bool;
}

/// Describes where the signal was defined. This is used for diagnostic warnings and is purely a
/// debug-mode tool.
pub trait DefinedAt {
    /// Returns the location at which the signal was defined. This is usually simply `None` in
    /// release mode.
    fn defined_at(&self) -> Option<&'static Location<'static>>;

    /// Returns the debug name of the signal, if any.
    fn debug_name(&self) -> Option<String> {
        None
    }
}

#[doc(hidden)]
pub fn panic_getting_disposed_signal(
    defined_at: Option<&'static Location<'static>>,
    debug_name: Option<String>,
    location: &'static Location<'static>,
) -> String {
    if let Some(name) = debug_name {
        if let Some(defined_at) = defined_at {
            format!(
                "At {location}, you tried to access a reactive value \"{name}\" which was \
                 defined at {defined_at}, but it has already been disposed."
            )
        } else {
            format!(
                "At {location}, you tried to access a reactive value \"{name}\", but it has \
                 already been disposed."
            )
        }
    } else {
        if let Some(defined_at) = defined_at {
            format!(
                "At {location}, you tried to access a reactive value which was \
                 defined at {defined_at}, but it has already been disposed."
            )
        } else {
            format!(
                "At {location}, you tried to access a reactive value, but it has \
                 already been disposed."
            )
        }
    }
}

/// Updates the value of the signal by replacing it, without notifying subscribers.
pub trait SetUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value by replacing it, non-reactively.
    ///
    /// If the signal has already been disposed, returns `Some(value)` with the value that was
    /// passed in. Otherwise, returns `None`.
    fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value>;

    /// Updates the value by replacing it, non-reactively.
    ///
    /// # Panics
    /// Panics if you try to set a signal that has been disposed.
    #[track_caller]
    fn set_untracked(&self, value: Self::Value) {
        if let Some(_) = self.try_set_untracked(value) {
            panic!(
                "{}",
                crate::traits::panic_getting_disposed_signal(
                    self.defined_at(),
                    self.debug_name(),
                    std::panic::Location::caller()
                )
            );
        }
    }
}

impl<T> SetUntracked for T
where
    T: UpdateUntracked + IsDisposed,
{
    type Value = <Self as UpdateUntracked>::Value;

    #[track_caller]
    fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value> {
        if self.is_disposed() {
            Some(value)
        } else {
            self.update_untracked(|n| *n = value);
            None
        }
    }
}
