use std::rc::Rc;

/// A wrapper around a reference-counted closure.
/// This is used to pass event handlers and callbacks to components.
#[derive(Clone)]
pub struct Callback<T = ()> {
    f: Rc<dyn Fn(T)>,
}

impl<T> Callback<T> {
    /// Create a new callback from a closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(T) + 'static,
    {
        Self { f: Rc::new(f) }
    }

    /// Call the callback.
    pub fn call(&self, arg: T) {
        (self.f)(arg);
    }
}

impl<T> std::fmt::Debug for Callback<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Callback")
    }
}

// Allow passing a closure directly where a Callback is expected (if Into is used)
impl<T, F> From<F> for Callback<T>
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

// Note: `Fn` traits are unstable to implement manually outside of nightly with features.
// So we probably shouldn't implement Fn directly if we want stable Rust.
// Silex seems to be using standard Rust, so skip Fn impl for now, unless the user uses nightly.
// The user is on Windows, environment unknown. Safer to stick to `impl From<F>`.
