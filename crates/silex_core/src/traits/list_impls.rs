use std::rc::Rc;

use crate::{
    error::{SilexError, SilexResult, handle_error},
    traits::{ForErrorHandler, ForLoopSource},
};

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

impl ForErrorHandler {
    pub fn call(&self, err: SilexError) {
        (self.0)(err);
    }
}

impl<F> From<F> for ForErrorHandler
where
    F: Fn(SilexError) + 'static,
{
    fn from(value: F) -> Self {
        Self(Rc::new(value))
    }
}

impl Default for ForErrorHandler {
    fn default() -> Self {
        Self(Rc::new(handle_error))
    }
}
