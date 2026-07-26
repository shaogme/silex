use std::any::{Any, type_name};
use std::marker::PhantomData;

use silex_reactivity::{CallbackId, callback};

#[cfg(debug_assertions)]
use crate::log::console_error;

/// A `Copy`-able wrapper for callbacks/event handlers.
///
/// This type uses a `NodeId` handle to reference a callback stored in the
/// reactive runtime, enabling `Copy` semantics similar to `Signal` and `Memo`.
///
/// # Example
///
/// ```rust,ignore
/// let cb = Callback::new(|x: i32| println!("Got: {}", x));
/// cb.call(42);
///
/// // Callback is Copy, so no need to clone
/// let cb2 = cb;
/// cb2.call(100);
/// ```
#[derive(Debug)]
pub struct Callback<T = ()> {
    id: CallbackId,
    marker: PhantomData<T>,
}

impl<T> Clone for Callback<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Callback<T> {}

impl<T: 'static> Callback<T> {
    /// Create a new callback from a closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(T) + 'static,
    {
        let id = callback::create(move |any: Box<dyn Any>| {
            if let Ok(arg) = any.downcast::<T>() {
                f(*arg);
            } else {
                #[cfg(debug_assertions)]
                {
                    let type_name = type_name::<T>();
                    console_error(
                        format!("Callback: type mismatch, expected {}", type_name).as_str(),
                    );
                }
            }
        });
        Self {
            id,
            marker: PhantomData,
        }
    }

    /// Call the callback with the given argument.
    pub fn call(&self, arg: T) {
        let _ = callback::invoke(self.id, Box::new(arg));
    }

    /// Returns the underlying handle for this callback.
    pub fn id(&self) -> CallbackId {
        self.id
    }
}

// Allow passing a closure directly where a Callback is expected (if Into is used)
impl<T: 'static, F> From<F> for Callback<T>
where
    F: Fn(T) + 'static,
{
    fn from(f: F) -> Self {
        Self::new(f)
    }
}

impl<T: 'static> Default for Callback<T> {
    fn default() -> Self {
        Self::new(|_| {})
    }
}
