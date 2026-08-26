use silex_core::{ReactiveError, SilexError, SilexResult};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub(crate) struct ActiveRegistrar<'scope> {
    inner: Rc<dyn Fn() -> bool + 'scope>,
}

impl<'scope> ActiveRegistrar<'scope> {
    pub(crate) fn new<F>(is_active: F) -> Self
    where
        F: Fn() -> bool + 'scope,
    {
        Self {
            inner: Rc::new(is_active),
        }
    }

    pub(crate) fn get(&self) -> bool {
        (self.inner)()
    }
}

#[doc(hidden)]
pub struct SharedCell<T> {
    pub(crate) inner: Rc<RefCell<T>>,
}

impl<T> Clone for SharedCell<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> SharedCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
        }
    }

    pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> R {
        callback(&self.inner.borrow())
    }

    pub fn with_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
        callback(&mut self.inner.borrow_mut())
    }

    pub fn replace(&self, value: T) -> T {
        self.inner.replace(value)
    }

    pub fn set(&self, value: T) {
        drop(self.replace(value));
    }

    pub fn take(&self) -> T
    where
        T: Default,
    {
        self.replace(T::default())
    }
}

pub struct MountState<'scope, T> {
    value: SharedCell<Option<T>>,
    active: ActiveRegistrar<'scope>,
}

impl<'scope, T: 'scope> MountState<'scope, T> {
    pub(crate) fn new(value: T, active: ActiveRegistrar<'scope>) -> Self {
        Self {
            value: SharedCell::new(Some(value)),
            active,
        }
    }

    fn ensure_access(&self) -> SilexResult<()> {
        if self.active.get() {
            Ok(())
        } else {
            Err(SilexError::fatal(ReactiveError::NoSuchNode))
        }
    }

    pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> SilexResult<R> {
        self.ensure_access()?;
        self.value.with(|value| {
            value
                .as_ref()
                .map(callback)
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
        })
    }

    pub fn update<R>(&self, callback: impl FnOnce(&mut T) -> R) -> SilexResult<R> {
        self.ensure_access()?;
        self.value.with_mut(|value| {
            value
                .as_mut()
                .map(callback)
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
        })
    }

    pub fn take(&self) -> SilexResult<T> {
        self.ensure_access()?;
        self.value
            .with_mut(Option::take)
            .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
    }

    pub fn replace(&self, value: T) -> SilexResult<Option<T>> {
        self.ensure_access()?;
        Ok(self.value.with_mut(|current| current.replace(value)))
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    #[doc(hidden)]
    pub fn take_for_cleanup(&self) -> Option<T> {
        self.value.with_mut(Option::take)
    }
}

impl<'scope, T: 'scope> Clone for MountState<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            active: self.active.clone(),
        }
    }
}
