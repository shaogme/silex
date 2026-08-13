//! Reactive node primitives owned by an execution scope.

use std::{cell::RefCell, fmt, marker::PhantomData, rc::Rc};

use crate::{
    CallbackInvokeResult, ErrorHandler, ReactiveError, ReactiveResult,
    error::map_callback_error,
    handle::{CallbackId, DerivedId, EffectId, MemoId, NodeRefId, SignalId, StoredId},
    internal::{
        RawId,
        value::{AnyValue, CallbackThunkError},
    },
    runtime::{self, RuntimeInput, ScopeState},
};

/// Options controlling the initial callback and one-shot behavior of a watcher.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WatchOptions {
    pub immediate: bool,
    pub once: bool,
}

impl WatchOptions {
    pub const fn immediate(self) -> Self {
        Self {
            immediate: true,
            ..self
        }
    }

    pub const fn once(self) -> Self {
        Self { once: true, ..self }
    }
}

// =============================================================================
// Callback
// =============================================================================

/// Scope-owned typed callbacks.
pub struct Callback<'scope, T, E = ReactiveError> {
    pub(crate) handle: CallbackId<'scope>,
    pub(crate) marker: PhantomData<fn(T) -> E>,
}

impl<T, E> Copy for Callback<'_, T, E> {}

impl<T, E> Clone for Callback<'_, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope, E: 'scope> Callback<'scope, T, E> {
    pub fn invoke(&self, arg: T) -> CallbackInvokeResult<(), E> {
        runtime::invoke_callback(&self.handle.state(), self.handle.raw(), AnyValue::new(arg))
            .map_err(map_callback_error)
    }

    pub fn dispatch(&self, arg: T, error_handler: ErrorHandler<'scope, E>) -> ReactiveResult<()> {
        match runtime::invoke_callback(&self.handle.state(), self.handle.raw(), AnyValue::new(arg))
        {
            Ok(()) => Ok(()),
            Err(CallbackThunkError::Runtime(error)) => Err(error),
            Err(CallbackThunkError::User(value)) => {
                let error = unsafe { value.downcast::<E>() }.ok_or(ReactiveError::TypeMismatch)?;
                error_handler.handle(error)
            }
        }
    }
}

// =============================================================================
// Effect
// =============================================================================

/// Scoped effects.
pub struct Effect<'scope> {
    pub(crate) handle: EffectId<'scope>,
}

impl Copy for Effect<'_> {}

impl Clone for Effect<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for Effect<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Effect").finish_non_exhaustive()
    }
}

impl<'scope> PartialEq for Effect<'scope> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<'scope> Eq for Effect<'scope> {}

impl<'scope> Effect<'scope> {
    pub fn stop(&self) -> ReactiveResult<bool> {
        runtime::stop_effect(&self.handle.state(), self.handle.raw())
    }
}

// =============================================================================
// Memo & Derived
// =============================================================================

/// Scoped lazy memo.
pub struct Memo<'scope, T, E> {
    pub(crate) handle: MemoId<'scope>,
    pub(crate) marker: PhantomData<fn() -> (T, E)>,
}

/// Scoped derived value whose callback may return a user-defined error.
pub struct Derived<'scope, T, E> {
    pub(crate) handle: DerivedId<'scope>,
    pub(crate) marker: PhantomData<fn() -> (T, E)>,
}

impl<'scope, T, E> Copy for Memo<'scope, T, E> {}

impl<'scope, T, E> Clone for Memo<'scope, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T, E> Copy for Derived<'scope, T, E> {}

impl<'scope, T, E> Clone for Derived<'scope, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope, E: 'scope> Memo<'scope, T, E> {
    pub fn get(&self) -> CallbackInvokeResult<T, E>
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn get_untracked(&self) -> CallbackInvokeResult<T, E>
    where
        T: Clone,
    {
        self.with_untracked(Clone::clone)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> CallbackInvokeResult<R, E> {
        runtime::with_fallible_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> CallbackInvokeResult<R, E> {
        runtime::with_fallible_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T: 'scope, E: 'scope> Derived<'scope, T, E> {
    pub fn get(&self) -> CallbackInvokeResult<T, E>
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn get_untracked(&self) -> CallbackInvokeResult<T, E>
    where
        T: Clone,
    {
        self.with_untracked(Clone::clone)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> CallbackInvokeResult<R, E> {
        runtime::with_fallible_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> CallbackInvokeResult<R, E> {
        runtime::with_fallible_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

// =============================================================================
// NodeRef
// =============================================================================

/// Scope-owned host object references.
pub struct NodeRef<'scope, T> {
    pub(crate) handle: NodeRefId<'scope>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<'scope, T> Copy for NodeRef<'scope, T> {}

impl<'scope, T> Clone for NodeRef<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: Clone + 'scope> NodeRef<'scope, T> {
    pub fn get(&self) -> ReactiveResult<Option<T>> {
        runtime::node_ref_get(&self.handle.state(), self.handle.raw())
    }
}

impl<'scope, T: 'scope> NodeRef<'scope, T> {
    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::node_ref_set(&self.handle.state(), self.handle.raw(), value)
    }

    pub fn clear(&self) -> ReactiveResult<()> {
        runtime::node_ref_clear::<T>(&self.handle.state(), self.handle.raw())
    }
}

// =============================================================================
// Signal
// =============================================================================

/// Read capability for a signal or memo-like node.
pub struct ReadSignal<'scope, T> {
    pub(crate) handle: SignalId<'scope>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// Write capability for a signal.
pub struct WriteSignal<'scope, T> {
    pub(crate) handle: SignalId<'scope>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// A paired read/write signal capability.
pub struct RwSignal<'scope, T> {
    pub(crate) read: ReadSignal<'scope, T>,
    pub(crate) write: WriteSignal<'scope, T>,
}

impl<'scope, T> Copy for ReadSignal<'scope, T> {}

impl<'scope, T> Clone for ReadSignal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for WriteSignal<'scope, T> {}

impl<'scope, T> Clone for WriteSignal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for RwSignal<'scope, T> {}

impl<'scope, T> Clone for RwSignal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope> ReadSignal<'scope, T> {
    pub fn get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T: 'scope> WriteSignal<'scope, T> {
    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let incoming = value;
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            *stored = incoming;
            (Ok(()), true)
        })?
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        let mut f = Some(f);
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            let Some(f) = f.take() else {
                return (Err(ReactiveError::Reentrant), false);
            };
            (Ok(f(stored)), true)
        })?
    }

    pub fn set_if_changed(&self, value: T) -> ReactiveResult<bool>
    where
        T: PartialEq,
    {
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let incoming = value;
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            if *stored == incoming {
                return (Ok(false), false);
            }
            *stored = incoming;
            (Ok(true), true)
        })?
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T: 'scope> RwSignal<'scope, T> {
    pub fn get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.read.get()
    }

    pub fn get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.read.get_untracked()
    }

    pub fn set(&self, value: T) -> ReactiveResult<()> {
        self.write.set(value)
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        self.write.update(f)
    }

    pub fn set_if_changed(&self, value: T) -> ReactiveResult<bool>
    where
        T: PartialEq,
    {
        self.write.set_if_changed(value)
    }

    pub fn read(&self) -> ReadSignal<'scope, T> {
        self.read
    }

    pub fn write(&self) -> WriteSignal<'scope, T> {
        self.write
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.read.runtime_input()
    }
}

/// Notify dependents after a silent update.
pub fn notify<'scope, T>(signal: &WriteSignal<'scope, T>) -> ReactiveResult<()> {
    let state = signal.handle.state();
    runtime::notify(&state, signal.handle.raw())
}

/// Track a read capability without reading its value.
pub fn track<'scope, T>(signal: &ReadSignal<'scope, T>) -> ReactiveResult<()> {
    let state = signal.handle.state();
    runtime::track(&state, signal.handle.raw())
}

/// Track multiple read capabilities in one call.
///
/// Handles from different `Runtime::run` or child-scope runs cannot be mixed in
/// one batch because their scope lifetimes are intentionally distinct. The
/// compile-fail case is covered by `tests/ui/fail_mixed_track_batch.rs`.
pub fn track_batch<'scope, T>(signals: &[ReadSignal<'scope, T>]) -> ReactiveResult<()> {
    let mut groups: Vec<(Rc<RefCell<ScopeState<'scope>>>, Vec<RawId>)> = Vec::new();

    for signal in signals {
        let state = signal.handle.state();
        if let Some((_group_state, ids)) = groups
            .iter_mut()
            .find(|(group_state, _)| Rc::ptr_eq(group_state, &state))
        {
            ids.push(signal.handle.raw());
        } else {
            groups.push((state, vec![signal.handle.raw()]));
        }
    }

    for (state, ids) in groups {
        runtime::track_many(&state, &ids)?;
    }

    Ok(())
}

// =============================================================================
// StoredValue
// =============================================================================

/// Scope-owned, non-reactive values.
///
/// During final disposal of the owning scope, this is the only node capability
/// that remains synchronously accessible: `with` and `update` may be used by a
/// pending cleanup before the payload is dropped.
/// The scope is still inactive, and the exception does not apply to effect
/// reruns, single-node stops, or asynchronous use after the cleanup returns.
pub struct StoredValue<'scope, T> {
    pub(crate) handle: StoredId<'scope>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<'scope, T> Copy for StoredValue<'scope, T> {}

impl<'scope, T> Clone for StoredValue<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope> StoredValue<'scope, T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_stored(&self.handle.state(), self.handle.raw(), |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        runtime::update_stored(&self.handle.state(), self.handle.raw(), |value| {
            unsafe { value.downcast_mut::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T, E> PartialEq for Memo<'scope, T, E> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T, E> Eq for Memo<'scope, T, E> {}

impl<'scope, T, E> PartialEq for Derived<'scope, T, E> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T, E> Eq for Derived<'scope, T, E> {}

impl<'scope, T> PartialEq for ReadSignal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T> Eq for ReadSignal<'scope, T> {}

impl<'scope, T> PartialEq for WriteSignal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T> Eq for WriteSignal<'scope, T> {}

impl<'scope, T> PartialEq for RwSignal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}
impl<'scope, T> Eq for RwSignal<'scope, T> {}

impl<'scope, T> PartialEq for StoredValue<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T> Eq for StoredValue<'scope, T> {}
