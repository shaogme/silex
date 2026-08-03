//! Root-owned node implementations and handle.

use crate::{
    ReactiveError, ReactiveResult,
    handle::{NodeKind, kind},
    internal::{RawId, value::AnyValue},
    root::scope::RootStateRef,
    runtime::{self, RuntimeInput, ScopeId},
};
use std::{
    marker::PhantomData,
    rc::{Rc, Weak},
};

pub(crate) struct OwnedHandle<K: NodeKind> {
    state: Weak<RootStateRef>,
    scope_id: ScopeId,
    raw: RawId,
    runtime_input: RuntimeInput,
    marker: PhantomData<fn() -> K>,
}

impl<K: NodeKind> Clone for OwnedHandle<K> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            scope_id: self.scope_id,
            raw: self.raw,
            runtime_input: self.runtime_input.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: NodeKind> OwnedHandle<K> {
    pub(crate) fn new(state: Weak<RootStateRef>, scope_id: ScopeId, raw: RawId) -> Self {
        let runtime_input = state
            .upgrade()
            .map(|state| RuntimeInput::from_scheduler(state.borrow().scheduler.clone()))
            .expect("root state must remain alive while creating a node handle");
        Self {
            state,
            scope_id,
            raw,
            runtime_input,
            marker: PhantomData,
        }
    }

    pub(crate) fn state(&self) -> ReactiveResult<Rc<RootStateRef>> {
        let state = self.state.upgrade().ok_or(ReactiveError::NoSuchNode)?;
        let active = state
            .borrow()
            .scheduler
            .borrow()
            .is_scope_active(self.scope_id);
        active.then_some(state).ok_or(ReactiveError::NoSuchNode)
    }

    pub(crate) fn raw(&self) -> RawId {
        self.raw
    }

    pub(crate) fn runtime_input(&self) -> RuntimeInput {
        self.runtime_input.clone()
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.state()
            .ok()
            .is_some_and(|state| state.borrow().node_kind(self.raw) == Some(K::TAG))
    }
}

pub struct RootCallback<T> {
    pub(crate) handle: OwnedHandle<kind::Callback>,
    pub(crate) marker: PhantomData<fn(T)>,
}

impl<T: 'static> RootCallback<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::Callback>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootCallback<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> RootCallback<T> {
    pub fn call(&self, value: T) -> bool {
        let Ok(state) = self.handle.state() else {
            return false;
        };
        runtime::invoke_callback(&state, self.handle.raw(), AnyValue::new(value)).is_ok()
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

pub struct RootEffect {
    pub(crate) handle: OwnedHandle<kind::Effect>,
}

impl RootEffect {
    pub(crate) fn new(handle: OwnedHandle<kind::Effect>) -> Self {
        Self { handle }
    }
}

impl Clone for RootEffect {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}

impl RootEffect {
    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

pub struct RootMemo<T> {
    pub(crate) handle: OwnedHandle<kind::Memo>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<T: 'static> RootMemo<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::Memo>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootMemo<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> RootMemo<T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        let state = self.handle.state()?;
        runtime::with_signal(&state, self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 root memo 失败")
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let state = self.handle.state().expect("读取 root memo 失败");
        runtime::with_signal(&state, self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
        .expect("读取 root memo 失败")
        .expect("读取 root memo 类型不匹配")
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        let state = self.handle.state()?;
        runtime::with_signal(&state, self.handle.raw(), false, |value| {
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

pub struct RootDerived<T> {
    pub(crate) handle: OwnedHandle<kind::Derived>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<T: 'static> RootDerived<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::Derived>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootDerived<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> RootDerived<T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        let state = self.handle.state()?;
        runtime::with_signal(&state, self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 root derived 失败")
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let state = self.handle.state().expect("读取 root derived 失败");
        runtime::with_signal(&state, self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
        .expect("读取 root derived 失败")
        .expect("读取 root derived 类型不匹配")
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        let state = self.handle.state()?;
        runtime::with_signal(&state, self.handle.raw(), false, |value| {
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

pub struct RootNodeRef<T> {
    pub(crate) handle: OwnedHandle<kind::NodeRef>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<T: 'static> RootNodeRef<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::NodeRef>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootNodeRef<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> RootNodeRef<T> {
    pub fn try_get(&self) -> ReactiveResult<Option<T>>
    where
        T: Clone,
    {
        let state = self.handle.state()?;
        runtime::node_ref_get(&state, self.handle.raw())
    }

    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.try_get().ok().flatten()
    }

    pub fn set(&self, value: T) -> ReactiveResult<()> {
        let state = self.handle.state()?;
        runtime::node_ref_set(&state, self.handle.raw(), value)
    }

    pub fn clear(&self) -> ReactiveResult<()> {
        let state = self.handle.state()?;
        runtime::node_ref_clear::<T>(&state, self.handle.raw())
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

pub struct RootReadSignal<T> {
    pub(crate) handle: OwnedHandle<kind::Signal>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<T: 'static> RootReadSignal<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::Signal>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

pub struct RootWriteSignal<T> {
    pub(crate) handle: OwnedHandle<kind::Signal>,
    pub(crate) marker: PhantomData<fn(T)>,
}

impl<T: 'static> RootWriteSignal<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::Signal>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

pub struct RootSignal<T> {
    read: RootReadSignal<T>,
    write: RootWriteSignal<T>,
}

impl<T: 'static> RootSignal<T> {
    pub(crate) fn new(read: RootReadSignal<T>, write: RootWriteSignal<T>) -> Self {
        Self { read, write }
    }
}

impl<T: 'static> Clone for RootReadSignal<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootWriteSignal<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootSignal<T> {
    fn clone(&self) -> Self {
        Self {
            read: self.read.clone(),
            write: self.write.clone(),
        }
    }
}

impl<T: 'static> RootReadSignal<T> {
    pub fn try_get(&self) -> ReactiveResult<T>
    where
        T: Clone,
    {
        let state = self.handle.state()?;
        runtime::with_signal(&state, self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .cloned()
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.try_get().expect("读取 root read signal 失败")
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let state = self.handle.state().expect("读取 root read signal 失败");
        runtime::with_signal(&state, self.handle.raw(), true, |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })
        .expect("读取 root read signal 失败")
        .expect("读取 root read signal 类型不匹配")
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        let state = self.handle.state()?;
        runtime::with_signal(&state, self.handle.raw(), false, |value| {
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

impl<T: 'static> RootWriteSignal<T> {
    pub fn try_set(&self, value: T) -> ReactiveResult<()> {
        let mut value = Some(value);
        let state = self.handle.state()?;
        runtime::update_signal(&state, self.handle.raw(), |stored| {
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            *stored = value.take().expect("root signal setter 只调用一次");
            (Ok(()), true)
        })?
    }

    pub fn set(&self, value: T) {
        let _ = self.try_set(value);
    }

    pub fn try_update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        let mut f = Some(f);
        let state = self.handle.state()?;
        runtime::update_signal(&state, self.handle.raw(), |stored| {
            let Some(stored) = (unsafe { stored.downcast_mut::<T>() }) else {
                return (Err(ReactiveError::TypeMismatch), false);
            };
            (
                Ok(f.take().expect("root signal updater 只调用一次")(
                    stored,
                )),
                true,
            )
        })?
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let _ = self.try_update(f);
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}

impl<T> RootSignal<T> {
    pub fn read(&self) -> RootReadSignal<T>
    where
        T: 'static,
    {
        self.read.clone()
    }

    pub fn write(&self) -> RootWriteSignal<T>
    where
        T: 'static,
    {
        self.write.clone()
    }

    pub fn get(&self) -> T
    where
        T: Clone + 'static,
    {
        self.read.get()
    }

    pub fn set(&self, value: T)
    where
        T: 'static,
    {
        self.write.set(value);
    }

    pub fn is_alive(&self) -> bool
    where
        T: 'static,
    {
        self.read.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput
    where
        T: 'static,
    {
        self.read.runtime_input()
    }
}

pub struct RootStoredValue<T> {
    pub(crate) handle: OwnedHandle<kind::Stored>,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<T: 'static> RootStoredValue<T> {
    pub(crate) fn new(handle: OwnedHandle<kind::Stored>) -> Self {
        Self {
            handle,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> Clone for RootStoredValue<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: 'static> RootStoredValue<T> {
    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
        let state = self.handle.state()?;
        runtime::with_stored(&state, self.handle.raw(), |value| {
            unsafe { value.downcast_ref::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("读取 root stored value 失败")
    }

    pub fn try_update<R>(&self, f: impl FnOnce(&mut T) -> R) -> ReactiveResult<R> {
        let state = self.handle.state()?;
        runtime::update_stored(&state, self.handle.raw(), |value| {
            unsafe { value.downcast_mut::<T>() }
                .map(f)
                .ok_or(ReactiveError::TypeMismatch)
        })?
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let _ = self.try_update(f);
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }

    #[doc(hidden)]
    pub fn runtime_input(&self) -> RuntimeInput {
        self.handle.runtime_input()
    }
}
