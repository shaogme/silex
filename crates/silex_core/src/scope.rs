//! High-level runtime and scope wrappers.

use crate::{
    Callback, NodeRef, Rx, SilexError, SilexResult, TaskHandle,
    reactivity::{Effect, Memo, ReactiveSource, ReadSignal, RwSignal, StoredValue, WriteSignal},
    task,
    traits::RxData,
};
use silex_reactivity::RuntimeInputs;
use std::{
    cell::{Cell, RefCell},
    future::Future,
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

    pub fn run(&mut self) -> RootHandle {
        let handle = self.inner.run();
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
    pub fn scope(&self) -> Scope<'_> {
        Scope {
            inner: self.inner.scope(),
        }
    }

    pub fn with_scope<'scope, R>(&'scope self, f: impl FnOnce(&Scope<'scope>) -> R) -> R {
        self.inner.with_scope(|scope| {
            let scope = Scope { inner: *scope };
            f(&scope)
        })
    }

    pub fn dispose(self) -> Result<(), silex_reactivity::CleanupError> {
        let Self { inner } = self;
        inner.dispose()
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_active()
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

    /// Create a memo without additional framework-declared inputs.
    pub fn memo<T, F>(&self, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.memo_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn memo_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.try_memo_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped memo 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_memo_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> SilexResult<Memo<'scope, T>>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.inner
            .try_memo_from(inputs, f)
            .map(|memo| Memo::from_inner(memo, *self))
            .map_err(|error| SilexError::Reactivity(error.to_string()))
    }

    /// Create a derived value without additional framework-declared inputs.
    pub fn derived<T, F>(&self, f: F) -> Rx<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        self.derived_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn derived_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> Rx<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        Rx::from_derived(self.inner.derived_from(inputs, f), *self)
    }

    #[doc(hidden)]
    pub fn try_derived_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> SilexResult<Rx<'scope, T>>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        self.inner
            .try_derived_from(inputs, f)
            .map(|derived| Rx::from_derived(derived, *self))
            .map_err(|error| SilexError::Reactivity(error.to_string()))
    }

    pub fn effect<F>(&self, f: F) -> Effect<'scope>
    where
        F: FnMut() + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> Effect<'scope>
    where
        F: FnMut() + 'scope,
    {
        self.try_effect_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped effect 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> SilexResult<Effect<'scope>>
    where
        F: FnMut() + 'scope,
    {
        let effect = self
            .inner
            .try_effect_from(inputs, f)
            .map_err(|error| SilexError::Reactivity(error.to_string()))?;
        Ok(Effect::from_inner(effect))
    }

    /// Create an effect that receives the value returned by its previous run.
    ///
    /// The first run receives `None`. A returned value is committed as the
    /// previous value for the next run; if the callback panics, no value is
    /// committed and the next run receives `None`.
    pub fn effect_with_previous<T, F>(&self, f: F) -> Effect<'scope>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn effect_with_previous_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> Effect<'scope>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        self.try_effect_with_previous_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped previous effect 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_effect_with_previous_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        mut f: F,
    ) -> SilexResult<Effect<'scope>>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        let previous = Rc::new(RefCell::new(None::<T>));
        let previous_for_effect = previous.clone();
        let effect = self
            .inner
            .try_effect_from(inputs, move || {
                let old = previous_for_effect.borrow_mut().take();
                let new = f(old);
                *previous_for_effect.borrow_mut() = Some(new);
            })
            .map_err(|error| SilexError::Reactivity(error.to_string()))?;
        Ok(Effect::from_inner(effect))
    }

    pub fn watch_from<W, T, C>(
        &self,
        inputs: RuntimeInputs,
        deps: W,
        callback: C,
        immediate: bool,
    ) -> Effect<'scope>
    where
        W: Fn() -> T + 'scope,
        T: Clone + PartialEq + 'scope,
        C: Fn(&T, Option<&T>, Option<()>) + 'scope,
    {
        let first_run = Rc::new(Cell::new(true));
        let previous = Rc::new(RefCell::new(None::<T>));
        self.effect_from(inputs, move || {
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

    pub fn callback<T, F>(&self, callback: F) -> Callback<'scope, T>
    where
        T: 'scope,
        F: FnMut(T) + 'scope,
    {
        let callback = self.inner.callback(callback);
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

    /// Spawn a task owned by this persistent scope or the currently running computation.
    pub fn spawn_scoped<F>(&self, future: F) -> TaskHandle
    where
        F: Future<Output = ()> + 'static,
    {
        if !self.is_active() {
            return TaskHandle::inactive();
        }
        let (task, cancel) = task::start(future);
        self.on_cleanup(cancel);
        task
    }

    /// Promote a source after validating its complete opaque input set.
    ///
    /// Plan materialization is the only step allowed to register target
    /// nodes, so a foreign input fails before any target mutation.
    pub fn try_promote<T>(&self, value: T) -> SilexResult<Rx<'scope, T::Value>>
    where
        T: ReactiveSource<'scope>,
        T::Value: Sized + RxData + 'scope,
    {
        value
            .into_promotion_plan()
            .materialize(self)
            .map_err(|error| SilexError::Reactivity(error.to_string()))
    }

    pub fn promote<T>(&self, value: T) -> Rx<'scope, T::Value>
    where
        T: ReactiveSource<'scope>,
        T::Value: Sized + RxData + 'scope,
    {
        self.try_promote(value)
            .unwrap_or_else(|error| panic!("reactive promotion failed: {error}"))
    }

    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.inner
            .try_validate_inputs(inputs)
            .map_err(|error| SilexError::Reactivity(error.to_string()))
    }

    pub(crate) fn assert_inputs(&self, inputs: &RuntimeInputs) {
        if let Err(error) = self.try_validate_inputs(inputs) {
            panic!("reactive input validation failed: {error}");
        }
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
///
/// This is intentionally not a general node-creation scope. It provides
/// owner-bound effect, cleanup, completion, child-owner, and disposal
/// operations; ordinary reactive nodes must be created through [`Scope`].
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

    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.inner
            .try_validate_inputs(inputs)
            .map_err(|error| SilexError::Reactivity(error.to_string()))
    }

    /// Register and immediately run an owner-bound effect without extra
    /// framework-declared inputs.
    pub fn effect<F>(&self, f: F) -> Effect<'_>
    where
        F: FnMut() + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f)
    }

    pub fn effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> Effect<'_>
    where
        F: FnMut() + 'scope,
    {
        self.try_effect_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 owned effect 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> SilexResult<Effect<'_>>
    where
        F: FnMut() + 'scope,
    {
        self.inner
            .try_effect_from(inputs, f)
            .map(Effect::from_inner)
            .map_err(|error| SilexError::Reactivity(error.to_string()))
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

    /// Spawn a task owned by this persistent scope or the currently running computation.
    pub fn spawn_scoped<F>(&self, future: F) -> TaskHandle
    where
        F: Future<Output = ()> + 'static,
    {
        if !self.is_active() {
            return TaskHandle::inactive();
        }
        let (task, cancel) = task::start(future);
        self.on_cleanup(cancel);
        task
    }

    pub fn dispose(&self) {
        self.inner.dispose();
    }
}
