//! A series of traits to implement the behavior of reactive primitive, especially signals.
//!
//! ## Principles
//! 1. **Composition**: Most of the traits are implemented as combinations of more primitive base traits,
//!    and blanket implemented for all types that implement those traits.
//! 2. **Fallibility**: Most traits includes a `try_` variant, which returns `None` if the method
//!    fails (e.g., if signals are arena allocated and this can't be found).
//!
//! ## Metadata Traits
//! - [`DefinedAt`] is used for debugging in the case of errors and should be implemented for all
//!   signal types.
//! - [`IsDisposed`] checks whether a signal is currently accessible.
//!
//! ## Base Traits
//! | Trait               | Mode    | Description                                                                           |
//! |---------------------|---------|---------------------------------------------------------------------------------------|
//! | [`Track`]           | —       | Tracks changes to this value, adding it as a source of the current reactive observer. |
//! | [`Notify`]          | —       | Notifies subscribers that this value has changed.                                     |
//! | [`WithUntracked`]   | Closure | Gives immutable access to the value of this signal without tracking.                  |
//! | [`UpdateUntracked`] | Closure | Gives mutable access to update the value of this signal without notifying.            |
//!
//! ## Derived Traits
//!
//! ### Access
//! | Trait             | Mode          | Composition                        | Description
//! |-------------------|---------------|------------------------------------|------------
//! | [`With`]          | `fn(&T) -> U` | [`WithUntracked`] + [`Track`]      | Applies closure to the current value of the signal and returns result, with reactive tracking.
//! | [`GetUntracked`]  | `T`           | [`WithUntracked`] + [`Clone`]      | Clones the current value of the signal.
//! | [`Get`]           | `T`           | [`With`] + [`Clone`]               | Clones the current value of the signal, with reactive tracking.
//! | [`Map`]           | `Memo<U>`     | [`Get`]                            | Creates a derived signal from this signal.
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
//! [`Get`] for `Clone` types).

// pub use crate::trait_options::*;

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
        // Op with T (Value)
        impl<T> std::ops::$trait<T> for $target<T>
        where
            T: std::ops::$trait<T, Output = T> + Clone + 'static,
            T: PartialEq + 'static,
        {
            type Output = $crate::reactivity::Memo<T>;

            fn $method(self, rhs: T) -> Self::Output {
                let lhs = self.clone();
                $crate::reactivity::Memo::new(move |_| {
                    use $crate::traits::Get;
                    lhs.get().$method(rhs.clone())
                })
            }
        }

        // Op with Reactives
        $crate::impl_reactive_op_rhs!($target, $trait, $method, $crate::reactivity::Signal<T>);
        $crate::impl_reactive_op_rhs!($target, $trait, $method, $crate::reactivity::ReadSignal<T>);
        $crate::impl_reactive_op_rhs!($target, $trait, $method, $crate::reactivity::Memo<T>);
        $crate::impl_reactive_op_rhs!($target, $trait, $method, $crate::reactivity::RwSignal<T>);
    };
}

#[macro_export]
macro_rules! impl_reactive_op_rhs {
    ($target:ident, $trait:ident, $method:ident, $rhs:ty) => {
        impl<T> std::ops::$trait<$rhs> for $target<T>
        where
            T: std::ops::$trait<T, Output = T> + Clone + 'static,
            T: PartialEq + 'static,
        {
            type Output = $crate::reactivity::Memo<T>;

            fn $method(self, rhs: $rhs) -> Self::Output {
                let lhs = self.clone();
                $crate::reactivity::Memo::new(move |_| {
                    use $crate::traits::Get;
                    lhs.get().$method(rhs.get())
                })
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
            type Output = $crate::reactivity::Memo<T>;

            fn $method(self) -> Self::Output {
                let lhs = self.clone();
                $crate::reactivity::Memo::new(move |_| {
                    use $crate::traits::Get;
                    lhs.get().$method()
                })
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

impl<F, T> GetUntracked for F
where
    F: Fn() -> T,
{
    type Value = T;

    fn try_get_untracked(&self) -> Option<Self::Value> {
        Some(self())
    }
}

impl<F, T> Get for F
where
    F: Fn() -> T,
    T: Clone,
{
    type Value = T;

    fn try_get(&self) -> Option<Self::Value> {
        Some(self())
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

        impl<$($T),*> GetUntracked for ($($T,)*)
        where
            $($T: GetUntracked),*
        {
            type Value = ($($T::Value,)*);

            fn try_get_untracked(&self) -> Option<Self::Value> {
                #[allow(non_snake_case)]
                let ($($T,)*) = self;
                Some(($($T.try_get_untracked()?,)*))
            }
        }

        impl<$($T),*> Get for ($($T,)*)
        where
            $($T: Get),*
        {
            type Value = ($($T::Value,)*);

            fn try_get(&self) -> Option<Self::Value> {
                #[allow(non_snake_case)]
                let ($($T,)*) = self;
                Some(($($T.try_get()?,)*))
            }
        }
    };
}

impl_tuple_traits!(A, B);
impl_tuple_traits!(A, B, C);
impl_tuple_traits!(A, B, C, D);

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: Get + Clone + 'static
where
    Self::Value: PartialEq + 'static,
{
    fn eq<O>(&self, other: O) -> crate::reactivity::Memo<bool>
    where
        O: Into<Self::Value> + Clone + 'static,
    {
        let other = other.into();
        let this = self.clone();
        crate::reactivity::Memo::new(move |_| this.get() == other)
    }

    fn ne<O>(&self, other: O) -> crate::reactivity::Memo<bool>
    where
        O: Into<Self::Value> + Clone + 'static,
    {
        let other = other.into();
        let this = self.clone();
        crate::reactivity::Memo::new(move |_| this.get() != other)
    }
}

impl<S> ReactivePartialEq for S
where
    S: Get + Clone + 'static,
    S::Value: PartialEq + 'static,
{
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: Get + Clone + 'static
where
    Self::Value: PartialOrd + 'static,
{
    fn gt<O>(&self, other: O) -> crate::reactivity::Memo<bool>
    where
        O: Into<Self::Value> + Clone + 'static,
    {
        let other = other.into();
        let this = self.clone();
        crate::reactivity::Memo::new(move |_| this.get() > other)
    }

    fn lt<O>(&self, other: O) -> crate::reactivity::Memo<bool>
    where
        O: Into<Self::Value> + Clone + 'static,
    {
        let other = other.into();
        let this = self.clone();
        crate::reactivity::Memo::new(move |_| this.get() < other)
    }

    fn ge<O>(&self, other: O) -> crate::reactivity::Memo<bool>
    where
        O: Into<Self::Value> + Clone + 'static,
    {
        let other = other.into();
        let this = self.clone();
        crate::reactivity::Memo::new(move |_| this.get() >= other)
    }

    fn le<O>(&self, other: O) -> crate::reactivity::Memo<bool>
    where
        O: Into<Self::Value> + Clone + 'static,
    {
        let other = other.into();
        let this = self.clone();
        crate::reactivity::Memo::new(move |_| this.get() <= other)
    }
}

impl<S> ReactivePartialOrd for S
where
    S: Get + Clone + 'static,
    S::Value: PartialOrd + 'static,
{
}

// use any_spawner::Executor;
// use futures::{Stream, StreamExt};
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

/// Clones the value of the signal, without tracking the value reactively.
pub trait GetUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

    /// Clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value>;

    /// Clones and returns the value of the signal,
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked()
            .unwrap_or_else(unwrap_signal!(self))
    }
}

/// Clones the value of the signal, without tracking the value reactively.
/// and subscribes the active reactive observer (an effect or computed) to changes in its value.
pub trait Get: DefinedAt {
    /// The type of the value contained in the signal.
    type Value: Clone;

    /// Subscribes to the signal, then clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get(&self) -> Option<Self::Value>;

    /// Subscribes to the signal, then clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(unwrap_signal!(self))
    }
}

/// Allows creating a derived signal from this signal.
pub trait Map: Sized {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Creates a derived signal from this signal.
    fn map<U, F>(self, f: F) -> crate::reactivity::Memo<U>
    where
        F: Fn(&Self::Value) -> U + 'static,
        U: Clone + PartialEq + 'static;
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
