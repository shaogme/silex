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
//! | [`WithUntracked`]   | Closure | Gives immutable access to the value of this signal.                                   |
//! | [`UpdateUntracked`] | Closure | Gives mutable access to update the value of this signal.                              |
//!
//! ## Derived Traits
//!
//! ### Access
//! | Trait             | Mode          | Composition                   | Description
//! |-------------------|---------------|-------------------------------|------------
//! | [`With`]          | `fn(&T) -> U` | [`WithUntracked`] + [`Track`]      | Applies closure to the current value of the signal and returns result, with reactive tracking.
//! | [`GetUntracked`]  | `T`           | [`WithUntracked`] + [`Clone`] | Clones the current value of the signal.
//! | [`Get`]           | `T`           | [`With`] + [`Clone`]          | Clones the current value of the signal, with reactive tracking.
//!
//! ### Update
//! | Trait               | Mode          | Composition                       | Description
//! |---------------------|---------------|-----------------------------------|------------
//! | [`Update`]          | `fn(&mut T)`  | [`UpdateUntracked`] + [`Notify`] | Applies closure to the current value to update it, and notifies subscribers.
//! | [`Set`]             | `T`           | [`Update`]                        | Sets the value to a new value, and notifies subscribers.

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
                    $crate::traits::panic_getting_disposed_signal($signal.defined_at(), location)
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

// === Metadata Traits ===

/// Describes where the signal was defined. This is used for diagnostic warnings and is purely a
/// debug-mode tool.
pub trait DefinedAt {
    /// Returns the location at which the signal was defined. This is usually simply `None` in
    /// release mode.
    fn defined_at(&self) -> Option<&'static Location<'static>>;
}

#[doc(hidden)]
pub fn panic_getting_disposed_signal(
    defined_at: Option<&'static Location<'static>>,
    location: &'static Location<'static>,
) -> String {
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

/// Checks whether a signal has already been disposed.
pub trait IsDisposed {
    /// If `true`, the signal cannot be accessed without a panic.
    fn is_disposed(&self) -> bool;
}

/// Allows disposing an arena-allocated signal before its owner has been disposed.
pub trait Dispose {
    /// Disposes of the signal.
    fn dispose(self);
}

// === Base Traits ===

/// Allows tracking the value of some reactive data.
pub trait Track {
    /// Subscribes to this signal in the current reactive scope without doing anything with its value.
    #[track_caller]
    fn track(&self);
}

/// Notifies subscribers of a change in this signal.
pub trait Notify {
    /// Notifies subscribers of a change in this signal.
    #[track_caller]
    fn notify(&self);
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

/// Updates the value of a signal by applying a function that updates it in place,
/// without notifying subscribers.
pub trait UpdateUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value by applying a function, returning the value returned by that function,
    /// or `None` if the signal has already been disposed.
    /// Does not notify subscribers that the signal has changed.
    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;

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
}

// === Derived Traits ===

// --- Access ---

/// Give read-only access to a signal's value by reference inside a closure,
/// and subscribes the active reactive observer.
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

impl<T> GetUntracked for T
where
    T: WithUntracked,
    T::Value: Clone,
{
    type Value = <Self as WithUntracked>::Value;

    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.try_with_untracked(Self::Value::clone)
    }
}

/// Clones the value of the signal, and subscribes the active reactive observer.
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

impl<T> Get for T
where
    T: With,
    T::Value: Clone,
{
    type Value = <T as With>::Value;

    #[track_caller]
    fn try_get(&self) -> Option<Self::Value> {
        self.try_with(Self::Value::clone)
    }
}

// --- Update ---

/// Updates the value of a signal by applying a function that updates it in place,
/// notifying its subscribers that the value has changed.
pub trait Update: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value of the signal and notifies subscribers.
    #[track_caller]
    fn update(&self, fun: impl FnOnce(&mut Self::Value)) {
        self.try_update(fun);
    }

    /// Updates the value of the signal and notifies subscribers, returning the value that is
    /// returned by the update function, or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;
}

impl<T> Update for T
where
    T: UpdateUntracked + Notify,
{
    type Value = <Self as UpdateUntracked>::Value;

    #[track_caller]
    fn try_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        // We allow the update to happen, and if it succeeds, we notify.
        // Ideally we might want to only notify if `fun` changed something,
        // but that requires `PartialEq` or return bool.
        // Here we just notify if update succeeded.
        let res = self.try_update_untracked(fun)?;
        self.notify();
        Some(res)
    }
}

/// Updates the value of the signal by replacing it.
pub trait Set: DefinedAt {
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

/// Turns a signal back into a raw value.
pub trait IntoInner {
    /// The type of the value contained in the signal.
    type Value;

    /// Returns the inner value if this is the only reference to the signal.
    /// Otherwise, returns `None` and drops this reference.
    fn into_inner(self) -> Option<Self::Value>;
}

// === StoredValue Traits ===

/// A variation of the [`Read`] trait that provides a signposted "always-non-reactive" API.
/// E.g. for [`StoredValue`](`crate::owner::StoredValue`).
pub trait ReadValue: Sized + DefinedAt {
    /// The guard type that will be returned, which can be dereferenced to the value.
    type Value: std::ops::Deref;

    /// Returns the non-reactive guard, or `None` if the value has already been disposed.
    #[track_caller]
    fn try_read_value(&self) -> Option<Self::Value>;

    /// Returns the non-reactive guard.
    ///
    /// # Panics
    /// Panics if you try to access a value that has been disposed.
    #[track_caller]
    fn read_value(&self) -> Self::Value {
        self.try_read_value().unwrap_or_else(unwrap_signal!(self))
    }
}

/// A variation of the [`With`] trait that provides a signposted "always-non-reactive" API.
/// E.g. for [`StoredValue`](`crate::owner::StoredValue`).
pub trait WithValue: DefinedAt {
    /// The type of the value contained in the value.
    type Value: ?Sized;

    /// Applies the closure to the value, non-reactively, and returns the result,
    /// or `None` if the value has already been disposed.
    #[track_caller]
    fn try_with_value<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    /// Applies the closure to the value, non-reactively, and returns the result.
    ///
    /// # Panics
    /// Panics if you try to access a value that has been disposed.
    #[track_caller]
    fn with_value<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with_value(fun)
            .unwrap_or_else(unwrap_signal!(self))
    }
}

impl<T> WithValue for T
where
    T: DefinedAt + ReadValue,
{
    type Value = <<Self as ReadValue>::Value as std::ops::Deref>::Target;

    fn try_with_value<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.try_read_value().map(|value| fun(&value))
    }
}

/// A variation of the [`Get`] trait that provides a signposted "always-non-reactive" API.
/// E.g. for [`StoredValue`](`crate::owner::StoredValue`).
pub trait GetValue: DefinedAt {
    /// The type of the value contained in the value.
    type Value: Clone;

    /// Clones and returns the value of the value, non-reactively,
    /// or `None` if the value has already been disposed.
    #[track_caller]
    fn try_get_value(&self) -> Option<Self::Value>;

    /// Clones and returns the value of the value, non-reactively.
    ///
    /// # Panics
    /// Panics if you try to access a value that has been disposed.
    #[track_caller]
    fn get_value(&self) -> Self::Value {
        self.try_get_value().unwrap_or_else(unwrap_signal!(self))
    }
}

impl<T> GetValue for T
where
    T: WithValue,
    T::Value: Clone,
{
    type Value = <Self as WithValue>::Value;

    fn try_get_value(&self) -> Option<Self::Value> {
        self.try_with_value(Self::Value::clone)
    }
}

/// A variation of the [`Write`] trait that provides a signposted "always-non-reactive" API.
/// E.g. for [`StoredValue`](`crate::owner::StoredValue`).
pub trait WriteValue: Sized + DefinedAt {
    /// The type of the value's value.
    type Value: Sized + 'static;

    /// Returns a non-reactive write guard, or `None` if the value has already been disposed.
    #[track_caller]
    fn try_write_value(&self) -> Option<impl std::ops::DerefMut<Target = Self::Value>>;

    /// Returns a non-reactive write guard.
    ///
    /// # Panics
    /// Panics if you try to access a value that has been disposed.
    #[track_caller]
    fn write_value(&self) -> impl std::ops::DerefMut<Target = Self::Value> {
        self.try_write_value().unwrap_or_else(unwrap_signal!(self))
    }
}

/// A variation of the [`Update`] trait that provides a signposted "always-non-reactive" API.
/// E.g. for [`StoredValue`](`crate::owner::StoredValue`).
pub trait UpdateValue: DefinedAt {
    /// The type of the value contained in the value.
    type Value;

    /// Updates the value, returning the value that is
    /// returned by the update function, or `None` if the value has already been disposed.
    #[track_caller]
    fn try_update_value<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;

    /// Updates the value.
    #[track_caller]
    fn update_value(&self, fun: impl FnOnce(&mut Self::Value)) {
        self.try_update_value(fun);
    }
}

impl<T> UpdateValue for T
where
    T: WriteValue,
{
    type Value = <Self as WriteValue>::Value;

    #[track_caller]
    fn try_update_value<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        let mut guard = self.try_write_value()?;
        Some(fun(&mut *guard))
    }
}

/// A variation of the [`Set`] trait that provides a signposted "always-non-reactive" API.
/// E.g. for [`StoredValue`](`crate::owner::StoredValue`).
pub trait SetValue: DefinedAt {
    /// The type of the value contained in the value.
    type Value;

    /// Updates the value by replacing it, non-reactively.
    ///
    /// If the value has already been disposed, returns `Some(value)` with the value that was
    /// passed in. Otherwise, returns `None`.
    #[track_caller]
    fn try_set_value(&self, value: Self::Value) -> Option<Self::Value>;

    /// Updates the value by replacing it, non-reactively.
    #[track_caller]
    fn set_value(&self, value: Self::Value) {
        self.try_set_value(value);
    }
}

impl<T> SetValue for T
where
    T: WriteValue,
{
    type Value = <Self as WriteValue>::Value;

    fn try_set_value(&self, value: Self::Value) -> Option<Self::Value> {
        // Unlike most other traits, for these None actually means success:
        if let Some(mut guard) = self.try_write_value() {
            *guard = value;
            None
        } else {
            Some(value)
        }
    }
}
