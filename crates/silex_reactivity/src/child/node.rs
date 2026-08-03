//! Reactive node primitives owned by an execution scope.

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use crate::{
    ReactiveError, ReactiveResult,
    handle::{CallbackId, DerivedId, EffectId, MemoId, NodeRefId, SignalId, StoredId},
    internal::{RawId, value::AnyValue},
    runtime::{self, ScopeState},
};

// =============================================================================
// Callback
// =============================================================================

/// Scope-owned type-erased callbacks.
pub struct Callback<'scope, 'run> {
    pub(crate) handle: CallbackId<'scope, 'run>,
}

impl Copy for Callback<'_, '_> {}

impl Clone for Callback<'_, '_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run> Callback<'scope, 'run> {
    pub fn invoke(&self, arg: AnyValue<'scope>) -> ReactiveResult<()> {
        // SAFETY: `arg` 仅在 `invoke_callback` 同步执行期间传给 `CallbackThunk`。
        // `CallbackThunk` 已被 extend_lifetime 存为 `'run`，但其内部闭包实际在 `'scope` 销毁。
        // 将 `arg` 提升至 `'run` 生命周期在同步调用内部是 sound 的。
        let arg = unsafe { std::mem::transmute::<AnyValue<'scope>, AnyValue<'run>>(arg) };
        runtime::invoke_callback(&self.handle.state(), self.handle.raw(), arg)
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

// =============================================================================
// Effect
// =============================================================================

/// Scoped effects.
pub struct Effect<'scope, 'run> {
    pub(crate) handle: EffectId<'scope, 'run>,
}

impl Copy for Effect<'_, '_> {}

impl Clone for Effect<'_, '_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl Effect<'_, '_> {
    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

// =============================================================================
// Memo & Derived
// =============================================================================

/// Scoped lazy memo.
pub struct Memo<'scope, 'run, T> {
    pub(crate) handle: MemoId<'scope, 'run>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// Scoped derived value.
pub struct Derived<'scope, 'run, T> {
    pub(crate) handle: DerivedId<'scope, 'run>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<'scope, 'run, T> Copy for Memo<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Memo<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for Derived<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Derived<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T: 'scope> Memo<'scope, 'run, T> {
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
}

impl<'scope, 'run, T: 'scope> Derived<'scope, 'run, T> {
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
}

// =============================================================================
// NodeRef
// =============================================================================

/// Scope-owned host object references.
pub struct NodeRef<'scope, 'run, T> {
    pub(crate) handle: NodeRefId<'scope, 'run>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<'scope, 'run, T> Copy for NodeRef<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for NodeRef<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T: Clone + 'scope> NodeRef<'scope, 'run, T> {
    pub fn try_get(&self) -> ReactiveResult<Option<T>> {
        runtime::node_ref_get(&self.handle.state(), self.handle.raw())
    }

    pub fn get(&self) -> Option<T> {
        self.try_get().ok().flatten()
    }
}

impl<'scope, 'run, T: 'scope> NodeRef<'scope, 'run, T> {
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
pub struct ReadSignal<'scope, 'run, T> {
    pub(crate) handle: SignalId<'scope, 'run>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// Write capability for a signal.
pub struct WriteSignal<'scope, 'run, T> {
    pub(crate) handle: SignalId<'scope, 'run>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

/// A paired read/write signal capability.
pub struct Signal<'scope, 'run, T> {
    pub(crate) read: ReadSignal<'scope, 'run, T>,
    pub(crate) write: WriteSignal<'scope, 'run, T>,
}

/// Alias used by callers that prefer the paired-signal terminology.
pub type RwSignal<'scope, 'run, T> = Signal<'scope, 'run, T>;

impl<'scope, 'run, T> Copy for ReadSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for ReadSignal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for WriteSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for WriteSignal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T> Copy for Signal<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Signal<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T: 'scope> ReadSignal<'scope, 'run, T> {
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
}

impl<'scope, 'run, T: 'scope> WriteSignal<'scope, 'run, T> {
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
}

impl<'scope, 'run, T: 'scope> Signal<'scope, 'run, T> {
    pub fn read(&self) -> ReadSignal<'scope, 'run, T> {
        self.read
    }

    pub fn write(&self) -> WriteSignal<'scope, 'run, T> {
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
}

/// Explicitly notify dependents after a silent update.
pub fn notify<'scope, 'run, T>(signal: &WriteSignal<'scope, 'run, T>) {
    let state = signal.handle.state();
    runtime::notify(&state, signal.handle.raw());
}

/// Track a read capability without reading its value.
pub fn track<'scope, 'run, T>(signal: &ReadSignal<'scope, 'run, T>) {
    let state = signal.handle.state();
    runtime::track(&state, signal.handle.raw());
}

/// Track multiple read capabilities in one call.
///
/// Handles from different `Runtime::run` or child-scope runs cannot be mixed in
/// one batch because their `'run` lifetimes are intentionally distinct. The
/// compile-fail case is covered by `tests/ui/fail_mixed_track_batch.rs`.
pub fn track_batch<'scope, 'run, T>(signals: &[ReadSignal<'scope, 'run, T>]) {
    let mut groups: Vec<(Rc<RefCell<ScopeState<'run>>>, Vec<RawId>)> = Vec::new();

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
pub struct StoredValue<'scope, 'run, T> {
    pub(crate) handle: StoredId<'scope, 'run>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<'scope, 'run, T> Copy for StoredValue<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for StoredValue<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T: 'scope> StoredValue<'scope, 'run, T> {
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
}

impl<'scope, 'run, T> PartialEq for Memo<'scope, 'run, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, 'run, T> Eq for Memo<'scope, 'run, T> {}

impl<'scope, 'run, T> PartialEq for Derived<'scope, 'run, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, 'run, T> Eq for Derived<'scope, 'run, T> {}

impl<'scope, 'run, T> PartialEq for ReadSignal<'scope, 'run, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, 'run, T> Eq for ReadSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> PartialEq for WriteSignal<'scope, 'run, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, 'run, T> Eq for WriteSignal<'scope, 'run, T> {}

impl<'scope, 'run, T> PartialEq for Signal<'scope, 'run, T> {
    fn eq(&self, other: &Self) -> bool {
        self.read == other.read && self.write == other.write
    }
}
impl<'scope, 'run, T> Eq for Signal<'scope, 'run, T> {}

impl<'scope, 'run, T> PartialEq for StoredValue<'scope, 'run, T> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl<'scope, 'run, T> Eq for StoredValue<'scope, 'run, T> {}
