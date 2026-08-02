//! Scoped signal primitives.

use crate::{
    ReactiveError, ReactiveResult,
    handle::{Handle, SignalId, kind},
    internal::{RawId, value::AnyValue},
    runtime::{self, ScopeState},
    scope::Scope,
};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

/// Read capability for a signal or memo-like node.
pub struct ReadSignal<'scope, 'run, T> {
    pub(crate) handle: SignalId<'scope, 'run>,
    marker: PhantomData<fn() -> T>,
}

/// Write capability for a signal.
pub struct WriteSignal<'scope, 'run, T> {
    pub(crate) handle: SignalId<'scope, 'run>,
    marker: PhantomData<fn() -> T>,
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

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create a signal owned by this scope.
    pub fn signal<T: 'static>(
        &self,
        value: T,
    ) -> (ReadSignal<'scope, 'run, T>, WriteSignal<'scope, 'run, T>) {
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_signal(AnyValue::new(value));
        let handle = Handle::new(self.frame, raw);
        (
            ReadSignal {
                handle,
                marker: PhantomData,
            },
            WriteSignal {
                handle,
                marker: PhantomData,
            },
        )
    }

    /// Create the paired form of a signal for callers that want one copyable
    /// capability instead of separate read/write values.
    pub fn rw_signal<T: 'static>(&self, value: T) -> Signal<'scope, 'run, T> {
        let (read, write) = self.signal(value);
        Signal { read, write }
    }
}

impl<'scope, 'run, T: Clone + 'static> ReadSignal<'scope, 'run, T> {
    pub fn try_get(&self) -> ReactiveResult<T> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            value
                .downcast_ref::<T>()
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T {
        self.try_get().expect("读取 scoped signal 失败")
    }

    pub fn try_get_untracked(&self) -> ReactiveResult<T> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            value
                .downcast_ref::<T>()
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 scoped signal 失败")
    }

    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), true, |value| {
            value
                .downcast_ref::<T>()
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        runtime::with_signal(&self.handle.state(), self.handle.raw(), false, |value| {
            value
                .downcast_ref::<T>()
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

impl<'scope, 'run, T: 'static> WriteSignal<'scope, 'run, T> {
    pub fn try_set(&self, value: T) -> ReactiveResult<()> {
        let mut value = Some(value);
        runtime::update_signal(&self.handle.state(), self.handle.raw(), |stored| {
            let Some(stored) = stored.downcast_mut::<T>() else {
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
            let Some(stored) = stored.downcast_mut::<T>() else {
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
            let Some(stored) = stored.downcast_mut::<T>() else {
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

impl<'scope, 'run, T: Clone + 'static> Signal<'scope, 'run, T> {
    pub fn read(&self) -> ReadSignal<'scope, 'run, T> {
        self.read
    }

    pub fn write(&self) -> WriteSignal<'scope, 'run, T> {
        self.write
    }

    pub fn get(&self) -> T {
        self.read.get()
    }

    pub fn set(&self, value: T)
    where
        T: 'static,
    {
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

// Keep the kind import used by rustdoc links and make accidental raw-handle
// reconstruction impossible outside this crate.
#[allow(dead_code)]
fn _signal_kind_marker(_: kind::Signal) {}
