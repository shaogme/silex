use crate::error::SilexResult;

/// Trait to unify different types of data sources that can be used in a `For` loop
/// via zero-copy slice access.
pub trait ForLoopSource {
    type Item: Clone;

    /// Returns a slice of the items.
    fn as_slice(&self) -> SilexResult<&[Self::Item]>;
}

// Impl for Vec<T>
impl<T: Clone + 'static> ForLoopSource for Vec<T> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self.as_slice())
    }
}

// Impl for Option<Vec<T>>
impl<T: Clone + 'static> ForLoopSource for Option<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Some(v) => Ok(v.as_slice()),
            None => Ok(&[]),
        }
    }
}

// Impl for SilexResult<Vec<T>>
impl<T: Clone + 'static> ForLoopSource for SilexResult<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Ok(v) => Ok(v.as_slice()),
            Err(e) => Err(e.clone()),
        }
    }
}

#[derive(Clone)]
pub struct ForErrorHandler(std::rc::Rc<dyn Fn(crate::SilexError)>);

impl ForErrorHandler {
    pub fn call(&self, err: crate::SilexError) {
        (self.0)(err);
    }
}

impl<F> From<F> for ForErrorHandler
where
    F: Fn(crate::SilexError) + 'static,
{
    fn from(value: F) -> Self {
        Self(std::rc::Rc::new(value))
    }
}

impl Default for ForErrorHandler {
    fn default() -> Self {
        Self(std::rc::Rc::new(crate::error::handle_error))
    }
}
