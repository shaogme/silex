//! Reactive node primitives owned by an execution scope.

use std::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    CallbackInvokeResult, ErrorContext, ErrorHandlerInput, HandlerError, ReactiveError,
    ReactiveResult,
    error::ErrorSlotRef,
    error::map_callback_error,
    handle::{CallbackId, ComputedId, EffectId, NodeRefId, SignalId, StoredId},
    internal::NodeId,
    runtime,
    runtime::storage::{CallbackThunk, CallbackThunkError, ReadLease, TypedNodeRef, WriteLease},
};

/// Options controlling the initial callback and one-shot behavior of a watcher.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WatchOptions {
    pub immediate: bool,
    pub once: bool,
}

/// Selects when an effect-like callback is rerun during a reactive flush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EffectPhase {
    /// Participate in the current synchronous reactive convergence.
    Normal,
    /// Run after normal work and deferred errors have converged.
    PostFlush,
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
        runtime::invoke_callback(
            &self.handle.state(),
            self.handle.raw(),
            self.callback.pointer(),
            arg,
        )
        .map_err(map_callback_error)
    }

    pub fn dispatch(
        &self,
        arg: T,
        error_handler: impl ErrorHandlerInput<'scope, E>,
    ) -> Result<(), HandlerError> {
        match runtime::invoke_callback(
            &self.handle.state(),
            self.handle.raw(),
            self.callback.pointer(),
            arg,
        ) {
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
    pub fn read(&self) -> CallbackInvokeResult<ReadGuard<'scope, T>, E> {
        let state = self.handle.state();
        runtime::read_fallible_signal_lease(&state, self.handle.raw(), self.value, self.errors)
            .map(|lease| ReadGuard::new(state, lease, true))
    }

    pub fn read_untracked(&self) -> CallbackInvokeResult<ReadGuard<'scope, T>, E> {
        let state = self.handle.state();
        runtime::read_fallible_signal_lease_untracked(
            &state,
            self.handle.raw(),
            self.value,
            self.errors,
        )
        .map(|lease| ReadGuard::new(state, lease, true))
    }

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
            |value| Ok(f(value)),
        )
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> CallbackInvokeResult<R, E> {
        runtime::with_fallible_signal_untracked(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            self.errors,
            |value| Ok(f(value)),
        )
    }

    pub fn track(&self) -> CallbackInvokeResult<(), E> {
        runtime::track_fallible_signal(
            &self.handle.state(),
            self.handle.raw(),
            self.value,
            self.errors,
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

/// A scoped immutable borrow of a reactive payload.
pub struct ReadGuard<'scope, T: ?Sized> {
    lease: Option<ReadLease<'scope, T>>,
    state: runtime::ScopeState<'scope>,
    should_flush: bool,
}

impl<'scope, T: ?Sized> ReadGuard<'scope, T> {
    pub(crate) fn new(
        state: runtime::ScopeState<'scope>,
        lease: ReadLease<'scope, T>,
        should_flush: bool,
    ) -> Self {
        Self {
            lease: Some(lease),
            state,
            should_flush,
        }
    }

    /// Release the borrow and report any pending scheduler error immediately.
    pub fn finish(mut self) -> ReactiveResult<()> {
        let lease = self.lease.take();
        drop(lease);
        runtime::finish_guard(&self.state, self.should_flush)
    }
}

#[expect(
    clippy::unreachable,
    reason = "finish consumes the guard and takes its lease; reaching this branch is an internal invariant violation"
)]
impl<T: ?Sized> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.lease.as_ref() {
            Some(lease) => lease,
            None => unreachable!("read guard was accessed after finish"),
        }
    }
}

impl<T: ?Sized> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        let lease = self.lease.take();
        if lease.is_none() {
            return;
        }
        drop(lease);
        if let Err(error) = runtime::finish_guard(&self.state, self.should_flush) {
            runtime::report_guard_failure(&self.state, error);
        }
    }
}

enum WriteCommit {
    Signal(NodeId),
    Stored,
}

/// A scoped mutable borrow of a reactive payload.
pub struct WriteGuard<'scope, T: ?Sized> {
    lease: Option<WriteLease<'scope, T>>,
    state: runtime::ScopeState<'scope>,
    commit: Option<WriteCommit>,
    should_flush: bool,
}

impl<'scope, T: ?Sized> WriteGuard<'scope, T> {
    pub(crate) fn new_signal(
        state: runtime::ScopeState<'scope>,
        lease: WriteLease<'scope, T>,
        id: NodeId,
    ) -> Self {
        Self {
            lease: Some(lease),
            state,
            commit: Some(WriteCommit::Signal(id)),
            should_flush: true,
        }
    }

    pub(crate) fn new_stored(
        state: runtime::ScopeState<'scope>,
        lease: WriteLease<'scope, T>,
        should_flush: bool,
    ) -> Self {
        Self {
            lease: Some(lease),
            state,
            commit: Some(WriteCommit::Stored),
            should_flush,
        }
    }

    fn commit_inner(&mut self) -> ReactiveResult<()> {
        let lease = self.lease.take();
        drop(lease);
        match self.commit.take() {
            Some(WriteCommit::Signal(id)) => runtime::commit_signal(&self.state, id)?,
            Some(WriteCommit::Stored) | None => {}
        }
        runtime::finish_guard(&self.state, self.should_flush)
    }

    /// Release the borrow, publish the change, and flush pending work.
    pub fn commit(mut self) -> ReactiveResult<()> {
        self.commit_inner()
    }

    /// Release the borrow without publishing a change.
    pub fn abort(mut self) -> ReactiveResult<()> {
        let lease = self.lease.take();
        drop(lease);
        self.commit.take();
        runtime::finish_guard(&self.state, self.should_flush)
    }
}

#[expect(
    clippy::unreachable,
    reason = "commit or abort consumes the guard and takes its lease; reaching this branch is an internal invariant violation"
)]
impl<T: ?Sized> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self.lease.as_ref() {
            Some(lease) => lease,
            None => unreachable!("write guard was accessed after finish"),
        }
    }
}

#[expect(
    clippy::unreachable,
    reason = "commit or abort consumes the guard and takes its lease; reaching this branch is an internal invariant violation"
)]
impl<T: ?Sized> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.lease.as_mut() {
            Some(lease) => lease,
            None => unreachable!("write guard was accessed after finish"),
        }
    }
}

impl<T: ?Sized> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        if self.lease.is_none() {
            return;
        }
        if let Err(error) = self.commit_inner() {
            runtime::report_guard_failure(&self.state, error);
        }
    }
}

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
pub struct Signal<'scope, T> {
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

impl<'scope, T> Copy for Signal<'scope, T> {}

impl<'scope, T> Clone for Signal<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope> ReadSignal<'scope, T> {
    pub fn read(&self) -> ReactiveResult<ReadGuard<'scope, T>> {
        let state = self.handle.state();
        let lease = runtime::read_signal_lease(&state, self.handle.raw(), self.value)?;
        Ok(ReadGuard::new(state, lease, true))
    }

    pub fn read_untracked(&self) -> ReactiveResult<ReadGuard<'scope, T>> {
        let state = self.handle.state();
        let lease = runtime::read_signal_lease_untracked(&state, self.handle.raw(), self.value)?;
        Ok(ReadGuard::new(state, lease, true))
    }

    pub fn get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        self.with_untracked(Clone::clone)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), self.value, f)
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal_untracked(&self.handle.state(), self.handle.raw(), self.value, f)
    }

    pub fn track(&self) -> ReactiveResult<()> {
        runtime::track_signal(&self.handle.state(), self.handle.raw(), self.value)
    }
}

impl<'scope, T: 'scope> WriteSignal<'scope, T> {
    pub fn write(&self) -> ReactiveResult<WriteGuard<'scope, T>> {
        let state = self.handle.state();
        let lease = runtime::write_signal_lease(&state, self.handle.raw(), self.value)?;
        Ok(WriteGuard::new_signal(state, lease, self.handle.raw()))
    }

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

impl<'scope, T> Signal<'scope, T> {
    /// Build a paired signal from read and write capabilities.
    ///
    /// The capabilities must identify the same runtime signal node. This
    /// method returns [`ReactiveError::InvariantViolation`] for a mismatched
    /// pair.
    pub fn from_pair(
        pair: (ReadSignal<'scope, T>, WriteSignal<'scope, T>),
    ) -> ReactiveResult<Self> {
        let (read, write) = pair;
        if read.handle != write.handle {
            return Err(ReactiveError::InvariantViolation);
        }
        Ok(Self { read, write })
    }

    /// Consume the paired signal and return its existing capabilities.
    pub fn into_pair(self) -> (ReadSignal<'scope, T>, WriteSignal<'scope, T>) {
        (self.read, self.write)
    }

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

    pub fn track(&self) -> ReactiveResult<()> {
        self.read.track()
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

    pub fn read_signal(&self) -> ReadSignal<'scope, T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<'scope, T> {
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
    pub fn read(&self) -> ReactiveResult<ReadGuard<'scope, T>> {
        let state = self.handle.state();
        let (lease, should_flush) =
            runtime::read_stored_lease(&state, self.handle.raw(), self.value)?;
        Ok(ReadGuard::new(state, lease, should_flush))
    }

    pub fn read_untracked(&self) -> ReactiveResult<ReadGuard<'scope, T>> {
        self.read()
    }

    pub fn write(&self) -> ReactiveResult<WriteGuard<'scope, T>> {
        let state = self.handle.state();
        let (lease, should_flush) =
            runtime::write_stored_lease(&state, self.handle.raw(), self.value)?;
        Ok(WriteGuard::new_stored(state, lease, should_flush))
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_stored(&self.handle.state(), self.handle.raw(), self.value, f)
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        self.with(f)
    }

    pub fn track(&self) -> ReactiveResult<()> {
        runtime::track_stored(&self.handle.state(), self.handle.raw(), self.value)
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

impl<'scope, T> PartialEq for Signal<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}
impl<'scope, T> Eq for Signal<'scope, T> {}

impl<'scope, T> PartialEq for StoredValue<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T> Eq for StoredValue<'scope, T> {}
