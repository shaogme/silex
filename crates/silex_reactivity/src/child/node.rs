//! Reactive node primitives owned by an execution scope.

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use crate::{
    ReactiveError, ReactiveResult,
    handle::{CallbackId, DerivedId, EffectId, MemoId, NodeRefId, SignalId, StoredId},
    internal::{RawId, value::AnyValue},
    runtime::{self, RuntimeInput, ScopeState},
};

// =============================================================================
// Callback
// =============================================================================

/// Scope-owned typed callbacks.
pub struct Callback<'scope, T> {
    pub(crate) handle: CallbackId<'scope>,
    pub(crate) marker: PhantomData<fn(T)>,
}

impl<T> Copy for Callback<'_, T> {}

impl<T> Clone for Callback<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope> Callback<'scope, T> {
    pub fn invoke(&self, arg: T) -> ReactiveResult<()> {
        runtime::invoke_callback(&self.handle.state(), self.handle.raw(), AnyValue::new(arg))
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
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

impl Effect<'_> {
    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

// =============================================================================
// Memo & Derived
// =============================================================================

/// Scoped lazy memo.
pub struct Memo<'scope, T> {
    pub(crate) handle: MemoId<'scope>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// Scoped derived value.
pub struct Derived<'scope, T> {
    pub(crate) handle: DerivedId<'scope>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<'scope, T> Copy for Memo<'scope, T> {}

impl<'scope, T> Clone for Memo<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for Derived<'scope, T> {}

impl<'scope, T> Clone for Derived<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T: 'scope> Memo<'scope, T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 scoped memo 失败")
    }

    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 scoped memo 失败")
    }

    pub fn try_with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        self.try_with_untracked(f)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T: 'scope> Derived<'scope, T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 scoped derived 失败")
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .expect("读取 scoped derived 的类型不匹配")
        })
        .expect("读取 scoped derived 失败")
    }

    pub fn try_with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        self.try_with_untracked(f)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
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
    pub fn try_get(&self) -> ReactiveResult<Option<T>> {
        runtime::node_ref_get(&self.handle.state(), self.handle.raw())
    }

    pub fn get(&self) -> Option<T> {
        self.try_get().ok().flatten()
    }
}

impl<'scope, T: 'scope> NodeRef<'scope, T> {
    pub fn set(&self, value: T) -> ReactiveResult<()> {
        runtime::node_ref_set(&self.handle.state(), self.handle.raw(), value)
    }

    pub fn clear(&self) -> ReactiveResult<()> {
        runtime::node_ref_clear::<T>(&self.handle.state(), self.handle.raw())
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
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
pub struct Signal<'scope, T> {
    pub(crate) read: ReadSignal<'scope, T>,
    pub(crate) write: WriteSignal<'scope, T>,
}

/// Alias used by callers that prefer the paired-signal terminology.
pub type RwSignal<'scope, T> = Signal<'scope, T>;

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
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 scoped signal 失败")
    }

    pub fn try_get_untracked(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 scoped signal 失败")
    }

    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
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

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T: 'scope> WriteSignal<'scope, T> {
    pub fn try_set(&self, value: T) -> ReactiveResult<()> {
        let mut value = Some(value);
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            *stored = value.take().expect("signal setter 只调用一次");
            (Ok(()), true)
        })?
    }

    pub fn set(&self, value: T) {
        if let Err(error) = self.try_set(value) {
            debug_assert!(!error.is_bug(), "写入 scoped signal 失败: {error}");
        }
    }

    pub fn try_update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        let mut f = Some(f);
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            (
                Ok(f.take().expect("signal updater 只调用一次")(stored)),
                true,
            )
        })?
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        if let Err(error) = self.try_update(f) {
            debug_assert!(!error.is_bug(), "更新 scoped signal 失败: {error}");
        }
    }

    pub fn try_set_if_changed(&self, value: T) -> ReactiveResult<bool>
    where
        T: PartialEq,
    {
        let mut value = Some(value);
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            let incoming = value.take().expect("signal setter 只调用一次");
            if *stored == incoming {
                return (Ok(false), false);
            }
            *stored = incoming;
            (Ok(true), true)
        })?
    }

    pub fn set_if_changed(&self, value: T)
    where
        T: PartialEq,
    {
        let _ = self.try_set_if_changed(value);
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T: 'scope> Signal<'scope, T> {
    pub fn read(&self) -> ReadSignal<'scope, T> {
        self.read
    }

    pub fn write(&self) -> WriteSignal<'scope, T> {
        self.write
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read.get()
    }

    pub fn set(&self, value: T) {
        self.write.set(value);
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.read.runtime_input()
    }
}

/// Explicitly notify dependents after a silent update.
pub fn notify<'scope, T>(signal: &WriteSignal<'scope, T>) {
    let state = signal.handle.state();
    runtime::notify(&state, signal.handle.raw());
}

/// Track a read capability without reading its value.
pub fn track<'scope, T>(signal: &ReadSignal<'scope, T>) {
    let state = signal.handle.state();
    runtime::track(&state, signal.handle.raw());
}

/// Track multiple read capabilities in one call.
///
/// Handles from different `Runtime::run` or child-scope runs cannot be mixed in
/// one batch because their scope lifetimes are intentionally distinct. The
/// compile-fail case is covered by `tests/ui/fail_mixed_track_batch.rs`.
pub fn track_batch<'scope, T>(signals: &[ReadSignal<'scope, T>]) {
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
        runtime::track_many(&state, &ids);
    }
}

// =============================================================================
// StoredValue
// =============================================================================

/// Scope-owned, non-reactive values.
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
    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_stored(&self.handle.state(), self.handle.raw(), |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 scoped stored value 失败")
    }

    pub fn try_update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        runtime::update_stored(&self.handle.state(), self.handle.raw(), |value| {
            unsafe { value.downcast_mut::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        self.try_update(f).expect("更新 scoped stored value 失败")
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<'scope, T> PartialEq for Memo<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T> Eq for Memo<'scope, T> {}

impl<'scope, T> PartialEq for Derived<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, T> Eq for Derived<'scope, T> {}

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
