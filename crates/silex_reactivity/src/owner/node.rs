//! Reactive node primitives owned by an execution scope.

use std::{fmt, marker::PhantomData};

use crate::{
    CallbackInvokeResult, ErrorContext, ErrorHandlerInput, HandlerError, ReactiveError,
    ReactiveResult,
    error::ErrorSlotRef,
    error::map_callback_error,
    handle::{CallbackId, ComputedId, EffectId, NodeRefId, SignalId, StoredId},
    runtime,
    runtime::storage::{CallbackThunk, CallbackThunkError, TypedNodeRef},
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
    pub(crate) callback: TypedNodeRef<'scope, CallbackThunk<'scope, T, E>>,
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
        runtime::invoke_callback(&self.handle.state(), self.handle.raw(), self.callback, arg)
            .map_err(map_callback_error)
    }

    pub fn dispatch(
        &self,
        arg: T,
        error_handler: impl ErrorHandlerInput<'scope, E>,
    ) -> Result<(), HandlerError> {
        match runtime::invoke_callback(&self.handle.state(), self.handle.raw(), self.callback, arg)
        {
            Ok(()) => Ok(()),
            Err(CallbackThunkError::Runtime(error)) => Err(HandlerError::new(
                error,
                ErrorContext::new("callback dispatch"),
            )),
            Err(CallbackThunkError::User(error)) => error_handler
                .handler_ref()
                .lease()
                .and_then(|handler| handler.handle(error)),
        }
    }
}

// =============================================================================
// EffectHandle
// =============================================================================

/// Scoped effects.
pub struct EffectHandle<'scope> {
    pub(crate) handle: EffectId<'scope>,
}

impl Copy for EffectHandle<'_> {}

impl Clone for EffectHandle<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for EffectHandle<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectHandle").finish_non_exhaustive()
    }
}

impl<'scope> PartialEq for EffectHandle<'scope> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<'scope> Eq for EffectHandle<'scope> {}

impl<'scope> EffectHandle<'scope> {
    pub fn stop(&self) -> ReactiveResult<bool> {
        runtime::stop_effect(&self.handle.state(), self.handle.raw())
    }
}

/// Unified computed value returned by the explicit computed APIs.
pub struct Computed<'scope, T, E> {
    pub(crate) handle: ComputedId<'scope>,
    pub(crate) value: TypedNodeRef<'scope, T>,
    pub(crate) errors: ErrorSlotRef<'scope, E>,
    pub(crate) marker: PhantomData<fn() -> (T, E)>,
}

impl<'scope, T, E> Copy for Computed<'scope, T, E> {}

impl<'scope, T, E> Clone for Computed<'scope, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope, E: 'scope> Computed<'scope, T, E> {
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
        runtime::with_fallible_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            self.errors,
            true,
            |value| Ok(f(value)),
        )
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> CallbackInvokeResult<R, E> {
        runtime::with_fallible_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            self.errors,
            false,
            |value| Ok(f(value)),
        )
    }
}

// =============================================================================
// NodeRef
// =============================================================================

/// Scope-owned host object references.
pub struct NodeRef<'scope, T> {
    pub(crate) handle: NodeRefId<'scope>,
    pub(crate) value: TypedNodeRef<'scope, Option<T>>,
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
        runtime::node_ref_get(&self.handle.state(), self.handle.raw(), self.value)
    }
}

impl<'scope, T: 'scope> NodeRef<'scope, T> {
    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::node_ref_set(&self.handle.state(), self.handle.raw(), self.value, value)
    }

    pub fn clear(&self) -> ReactiveResult<()> {
        runtime::node_ref_clear(&self.handle.state(), self.handle.raw(), self.value)
    }
}

// =============================================================================
// Signal
// =============================================================================

/// Read capability for a signal or memo-like node.
pub struct ReadSignal<'scope, T> {
    pub(crate) handle: SignalId<'scope>,
    pub(crate) value: TypedNodeRef<'scope, T>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// Write capability for a signal.
pub struct WriteSignal<'scope, T> {
    pub(crate) handle: SignalId<'scope>,
    pub(crate) value: TypedNodeRef<'scope, T>,
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
        runtime::with_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            true,
            Clone::clone,
        )
    }

    pub fn get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            false,
            Clone::clone,
        )
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), self.value, true, f)
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            false,
            f,
        )
    }
}

impl<'scope, T: 'scope> WriteSignal<'scope, T> {
    pub fn notify(&self) -> ReactiveResult<()> {
        runtime::notify(&self.handle.state(), self.handle.raw())
    }

    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::update_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            |stored| {
                *stored = value;
                ((), true)
            },
        )
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        runtime::update_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            |stored| (f(stored), true),
        )
    }

    pub fn set_if_changed(&self, value: T) -> ReactiveResult<bool>
    where
        T: PartialEq,
    {
        runtime::update_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            |stored| {
                let incoming = value;
                if *stored == incoming {
                    return (false, false);
                }
                *stored = incoming;
                (true, true)
            },
        )
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
    pub(crate) value: TypedNodeRef<'scope, T>,
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
        runtime::with_stored(&self.handle.state(), self.handle.raw(), self.value, f)
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        runtime::update_stored(&self.handle.state(), self.handle.raw(), self.value, f)
    }
}

impl<'scope, T, E> PartialEq for Computed<'scope, T, E> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T, E> Eq for Computed<'scope, T, E> {}

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
