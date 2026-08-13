use silex_core::{ReactiveError, SilexError, SilexResult, StoredValue};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
pub(super) struct ActiveRegistrar<'scope> {
    inner: Rc<dyn Fn() -> bool + 'scope>,
}

impl<'scope> ActiveRegistrar<'scope> {
    pub(super) fn new<F>(is_active: F) -> Self
    where
        F: Fn() -> bool + 'scope,
    {
        Self {
            inner: Rc::new(is_active),
        }
    }

    pub(super) fn get(&self) -> bool {
        (self.inner)()
    }
}

/// A host resource cancellation handle owned by a view scope.
///
/// The handle deliberately exposes no reactive capability. Cancellation is
/// idempotent and the owner retains a clone so dropping this value early does
/// not transfer lifecycle ownership away from the view.
pub(super) type ResourceGate = Rc<Cell<bool>>;

/// Shared mutable state used by generated code and host resources.
#[doc(hidden)]
pub struct SharedCell<T> {
    pub(super) inner: Rc<RefCell<T>>,
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

/// Owner-bound mutable state that can only be accessed through closures.
///
/// The state rejects access after its owner becomes inactive. The framework
/// uses the cleanup-only methods while an owner is being disposed so cleanup
/// can still take the final value after the runtime has rejected new work.
/// States created for a lexical `Scope` are backed by that scope's
/// `StoredValue`, while states created for an `OwnedScope` use the local
/// fallback because an owned view owner intentionally cannot create nodes.
pub struct MountState<'scope, T> {
    value: MountStateValue<'scope, T>,
    active: ActiveRegistrar<'scope>,
}

enum MountStateValue<'scope, T> {
    Shared(SharedCell<Option<T>>),
    Stored(StoredValue<'scope, Option<T>>),
}

impl<'scope, T> Clone for MountStateValue<'scope, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Shared(value) => Self::Shared(value.clone()),
            Self::Stored(value) => Self::Stored(*value),
        }
    }
}

impl<'scope, T> Clone for MountState<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            active: self.active.clone(),
        }
    }
}

impl<'scope, T: 'scope> MountState<'scope, T> {
    pub(super) fn new(value: T, active: ActiveRegistrar<'scope>) -> Self {
        Self {
            value: MountStateValue::Shared(SharedCell::new(Some(value))),
            active,
        }
    }

    pub(super) fn new_stored(
        value: StoredValue<'scope, Option<T>>,
        active: ActiveRegistrar<'scope>,
    ) -> Self {
        Self {
            value: MountStateValue::Stored(value),
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
        match &self.value {
            MountStateValue::Shared(value) => value.with(|value| {
                value
                    .as_ref()
                    .map(callback)
                    .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
            }),
            MountStateValue::Stored(value) => value
                .with(|value| value.as_ref().map(callback))?
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
        }
    }

    pub fn update<R>(&self, callback: impl FnOnce(&mut T) -> R) -> SilexResult<R> {
        self.ensure_access()?;
        match &self.value {
            MountStateValue::Shared(value) => value.with_mut(|value| {
                value
                    .as_mut()
                    .map(callback)
                    .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
            }),
            MountStateValue::Stored(value) => value
                .update(|value| value.as_mut().map(callback))
                .map_err(SilexError::fatal)?
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
        }
    }

    pub fn take(&self) -> SilexResult<T> {
        self.ensure_access()?;
        match &self.value {
            MountStateValue::Shared(value) => value
                .with_mut(Option::take)
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
            MountStateValue::Stored(value) => value
                .update(Option::take)
                .map_err(SilexError::fatal)?
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
        }
    }

    pub fn replace(&self, value: T) -> SilexResult<Option<T>> {
        self.ensure_access()?;
        match &self.value {
            MountStateValue::Shared(current) => {
                Ok(current.with_mut(|current| current.replace(value)))
            }
            MountStateValue::Stored(current) => current
                .update(|current| current.replace(value))
                .map_err(SilexError::fatal),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    #[doc(hidden)]
    pub fn take_for_cleanup(&self) -> Option<T> {
        match &self.value {
            MountStateValue::Shared(value) => value.with_mut(Option::take),
            MountStateValue::Stored(value) => value.update(Option::take).ok().flatten(),
        }
    }
}
