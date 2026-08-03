//! High-level runtime and scope wrappers.

use crate::{
    Callback, NodeRef, Rx,
    reactivity::{Effect, Memo, ReadSignal, RwSignal, StoredValue, WriteSignal},
    traits::{IntoRx, RxData},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// User-owned high-level runtime.
pub struct Runtime {
    inner: silex_reactivity::Runtime,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            inner: silex_reactivity::Runtime::new(),
        }
    }

    pub fn run<F>(&mut self, f: F) -> RootHandle
    where
        F: FnOnce(&RootScope),
    {
        let handle = self.inner.run(move |scope| {
            let scope = RootScope {
                inner: scope.clone(),
            };
            f(&scope);
        });
        RootHandle { inner: handle }
    }

    pub fn child<R>(&mut self, f: impl for<'scope> FnOnce(Scope<'scope>) -> R) -> R {
        self.inner.child(|s| f(Scope { inner: *s }))
    }
}

pub struct RootHandle {
    inner: silex_reactivity::RootHandle,
}

impl RootHandle {
    pub fn scope(&self) -> RootScope {
        RootScope {
            inner: self.inner.scope(),
        }
    }

    pub fn dispose(&mut self) -> Result<(), silex_reactivity::CleanupError> {
        self.inner.dispose()
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }
}

#[derive(Clone)]
pub struct RootScope {
    inner: silex_reactivity::RootScope,
}

impl RootScope {
    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    pub fn signal<T: 'static>(
        &self,
        value: T,
    ) -> (
        silex_reactivity::RootReadSignal<T>,
        silex_reactivity::RootWriteSignal<T>,
    ) {
        self.inner.signal(value)
    }

    pub fn rw_signal<T: 'static>(&self, value: T) -> silex_reactivity::RootSignal<T> {
        self.inner.rw_signal(value)
    }

    pub fn effect<F>(&self, f: F) -> silex_reactivity::RootEffect
    where
        F: FnMut() + 'static,
    {
        self.inner.effect(f)
    }

    pub fn memo<T, F>(&self, f: F) -> silex_reactivity::RootMemo<T>
    where
        T: PartialEq + 'static,
        F: FnMut(Option<&T>) -> T + 'static,
    {
        self.inner.memo(f)
    }

    pub fn derived<T, F>(&self, f: F) -> silex_reactivity::RootDerived<T>
    where
        T: 'static,
        F: FnMut() -> T + 'static,
    {
        self.inner.derived(f)
    }

    pub fn stored<T: 'static>(&self, value: T) -> silex_reactivity::RootStoredValue<T> {
        self.inner.stored(value)
    }

    pub fn callback<T, F>(&self, callback: F) -> silex_reactivity::RootCallback<T>
    where
        T: 'static,
        F: FnMut(T) + 'static,
    {
        self.inner.callback(callback)
    }

    pub fn node_ref<T: 'static>(&self) -> silex_reactivity::RootNodeRef<T> {
        self.inner.node_ref()
    }

    pub fn completion<T, F>(&self, callback: F) -> silex_reactivity::CompletionToken<T>
    where
        T: 'static,
        F: FnMut(T) + 'static,
    {
        self.inner.completion(callback)
    }

    pub fn on_cleanup<F>(&self, cleanup: F)
    where
        F: FnOnce() + 'static,
    {
        self.inner.on_cleanup(cleanup);
    }

    pub fn on_dispose<F>(&self, hook: F)
    where
        F: FnOnce() + 'static,
    {
        self.inner.on_dispose(hook);
    }

    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        self.inner.untrack(f)
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        self.inner.batch(f)
    }

    pub fn child<R>(&self, f: impl for<'scope> FnOnce(Scope<'scope>) -> R) -> R {
        self.inner.child(|scope| f(Scope { inner: *scope }))
    }

    pub fn owned_scope(&self) -> OwnedScope<'static> {
        OwnedScope {
            inner: self.inner.owned_scope(),
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level scope capability. Its lifetimes are inherited from the
/// underlying runtime scope and are part of every node-bearing return type.
#[derive(Clone, Copy)]
pub struct Scope<'scope> {
    pub(crate) inner: silex_reactivity::Scope<'scope>,
}

impl<'scope> PartialEq for Scope<'scope> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<'scope> Eq for Scope<'scope> {}

impl<'scope> Scope<'scope> {
    pub fn owned_scope(&self) -> OwnedScope<'scope> {
        OwnedScope {
            inner: self.inner.owned_scope(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    pub fn signal<T: 'scope>(&self, value: T) -> (ReadSignal<'scope, T>, WriteSignal<'scope, T>) {
        let (read, write) = self.inner.signal(value);
        (
            ReadSignal::from_inner(read, *self),
            WriteSignal::from_inner(write),
        )
    }

    pub fn rw_signal<T: 'scope>(&self, value: T) -> RwSignal<'scope, T> {
        let (read, write) = self.signal(value);
        RwSignal::from_parts(read, write)
    }

    pub fn memo<T, F>(&self, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        Memo::from_inner(self.inner.memo(f), *self)
    }

    pub fn derived<T, F>(&self, f: F) -> Rx<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        Rx::from_derived(self.inner.derived(f), *self)
    }

    pub fn effect<T, F>(&self, mut f: F) -> Effect<'scope>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        let previous = Rc::new(RefCell::new(None::<T>));
        let previous_for_effect = previous.clone();
        let effect = self.inner.effect(move || {
            let old = previous_for_effect.borrow_mut().take();
            let new = f(old);
            *previous_for_effect.borrow_mut() = Some(new);
        });
        Effect::from_inner(effect)
    }

    pub fn watch<W, T, C>(&self, deps: W, callback: C, immediate: bool) -> Effect<'scope>
    where
        W: Fn() -> T + 'scope,
        T: Clone + PartialEq + 'scope,
        C: Fn(&T, Option<&T>, Option<()>) + 'scope,
    {
        let first_run = Rc::new(Cell::new(true));
        let previous = Rc::new(RefCell::new(None::<T>));
        self.effect(move |_: Option<()>| {
            let value = deps();
            let mut old_value = previous.borrow_mut();
            let old = old_value.clone();
            if first_run.replace(false) {
                *old_value = Some(value.clone());
                if immediate {
                    callback(&value, old.as_ref(), None);
                }
            } else if old.as_ref() != Some(&value) {
                callback(&value, old.as_ref(), None);
                *old_value = Some(value.clone());
            }
        })
    }

    pub fn stored<T: 'scope>(&self, value: T) -> StoredValue<'scope, T> {
        StoredValue::from_inner(self.inner.stored(value), *self)
    }

    pub fn callback<T, F>(&self, mut callback: F) -> Callback<'scope, T>
    where
        T: 'scope,
        F: FnMut(T) + 'scope,
    {
        let callback = self.inner.callback(move |value| {
            // SAFETY: this wrapper only invokes the underlying callback with
            // the same `T` through `Callback::call`.
            if let Some(value) = unsafe { value.downcast::<T>() } {
                callback(value);
            }
        });
        Callback::from_inner(callback)
    }

    pub fn node_ref<T: 'scope>(&self) -> NodeRef<'scope, T> {
        NodeRef::from_inner(self.inner.node_ref())
    }

    pub fn completion<T, F>(&self, callback: F) -> silex_reactivity::CompletionToken<T>
    where
        T: 'static,
        F: FnMut(T) + 'scope,
    {
        self.inner.completion(callback)
    }

    pub fn rx<T>(&self, value: T) -> Rx<'scope, T::Value>
    where
        T: IntoRx<'scope>,
        T::Value: Sized + RxData,
    {
        value.into_rx(self)
    }

    pub fn constant<T: 'scope>(&self, value: T) -> Rx<'scope, T> {
        let stored = self.stored(value);
        Rx::from_stored(stored)
    }

    pub fn child<R>(&self, f: impl for<'child> FnOnce(Scope<'child>) -> R) -> R {
        self.inner.child(|scope| f(Scope { inner: *scope }))
    }

    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        self.inner.untrack(f)
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        self.inner.batch(f)
    }

    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce() + 'scope,
    {
        self.inner.on_cleanup(f);
    }
}

/// Persistent owner used by dynamic branches and list rows.
pub struct OwnedScope<'scope> {
    inner: silex_reactivity::OwnedScope<'scope>,
}

impl<'scope> OwnedScope<'scope> {
    pub fn child(&self) -> Self {
        Self {
            inner: self.inner.child(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    pub fn effect<F>(&self, f: F)
    where
        F: FnMut() + 'scope,
    {
        self.inner.effect(f);
    }

    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce() + 'scope,
    {
        self.inner.on_cleanup(f);
    }

    pub fn completion<T, F>(&self, callback: F) -> silex_reactivity::CompletionToken<T>
    where
        T: 'static,
        F: FnMut(T) + 'scope,
    {
        self.inner.completion(callback)
    }

    pub fn dispose(&self) {
        self.inner.dispose();
    }
}
