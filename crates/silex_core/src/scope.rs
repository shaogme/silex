//! High-level runtime and scope wrappers.

use crate::{
    Callback, CompletionOnce, CompletionSender, ErrorHandler, ErrorReporter, NodeRef, Rx,
    SilexError, SilexResult, TaskHandle,
    reactivity::{
        Effect, Memo, ReactiveSource, ReadSignal, RwSignal, StoredValue, WatchOptions, WriteSignal,
    },
    task,
    traits::RxData,
};
#[cfg(feature = "test-support")]
use silex_reactivity::RuntimeSnapshot;
use silex_reactivity::{EffectInitError, RuntimeInputs};
use std::{future::Future, panic::UnwindSafe};

fn map_effect_init_error(error: EffectInitError<SilexError>) -> SilexError {
    match error {
        EffectInitError::Registration(error) => SilexError::from(error),
        EffectInitError::Initial(error) => error,
    }
}

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

    pub fn try_run(&mut self) -> SilexResult<RootHandle> {
        self.inner
            .try_run()
            .map(|handle| RootHandle { inner: handle })
            .map_err(SilexError::from)
    }

    pub fn child<R>(&mut self, f: impl for<'scope> FnOnce(Scope<'scope>) -> R) -> R {
        self.inner.child(|s| f(Scope { inner: s }))
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

    pub fn with_scope<'scope, R>(&'scope self, f: impl FnOnce(Scope<'scope>) -> R) -> R {
        self.inner.with_scope(|scope| f(Scope { inner: scope }))
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
    /// Register an application error destination owned by this scope.
    pub fn error_handler<F>(self, handler: F) -> ErrorReporter<'scope>
    where
        F: Fn(SilexError) + 'scope,
    {
        self.inner.error_handler(handler)
    }

    pub fn owned_scope(self) -> OwnedScope<'scope> {
        self.try_owned_scope()
            .unwrap_or_else(|error| panic!("创建 owned scope 失败: {error}"))
    }

    pub fn try_owned_scope(self) -> SilexResult<OwnedScope<'scope>> {
        self.inner
            .try_owned_scope()
            .map(|inner| OwnedScope { inner })
            .map_err(SilexError::from)
    }

    pub fn is_active(self) -> bool {
        self.inner.is_active()
    }

    pub fn signal<T: 'scope>(self, value: T) -> (ReadSignal<'scope, T>, WriteSignal<'scope, T>) {
        let (read, write) = self.inner.signal(value);
        (
            ReadSignal::from_inner(read, self),
            WriteSignal::from_inner(write),
        )
    }

    pub fn rw_signal<T: 'scope>(self, value: T) -> RwSignal<'scope, T> {
        let (read, write) = self.signal(value);
        RwSignal::from_parts(read, write)
    }

    /// Create a memo without additional framework-declared inputs.
    pub fn memo<T, F>(self, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.memo_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn memo_from<T, F>(self, inputs: RuntimeInputs, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.try_memo_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped memo 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_memo_from<T, F>(self, inputs: RuntimeInputs, f: F) -> SilexResult<Memo<'scope, T>>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.inner
            .try_memo_from(inputs, f)
            .map(|memo| Memo::from_inner(memo, self))
            .map_err(SilexError::from)
    }

    /// Create a derived value without additional framework-declared inputs.
    pub fn derived<T, F>(self, f: F) -> Rx<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        self.derived_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn derived_from<T, F>(self, inputs: RuntimeInputs, f: F) -> Rx<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        Rx::from_derived(self.inner.derived_from(inputs, f), self)
    }

    #[doc(hidden)]
    pub fn try_derived_from<T, F>(self, inputs: RuntimeInputs, f: F) -> SilexResult<Rx<'scope, T>>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        self.inner
            .try_derived_from(inputs, f)
            .map(|derived| Rx::from_derived(derived, self))
            .map_err(SilexError::from)
    }

    pub fn effect<F>(
        self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'scope>>
    where
        F: FnMut() -> SilexResult<()> + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f, error_handler)
    }

    #[doc(hidden)]
    pub fn effect_from<F>(
        self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'scope>>
    where
        F: FnMut() -> SilexResult<()> + 'scope,
    {
        let effect = self
            .inner
            .effect_from(inputs, f, error_handler)
            .map_err(map_effect_init_error)?;
        Ok(Effect::from_inner(effect))
    }

    /// Create an effect that receives the value returned by its previous run.
    ///
    /// The first run receives `None`. A returned value is committed as the
    /// previous value for the next run; if the callback fails or panics, the
    /// last successfully committed value remains available for retry.
    pub fn effect_with_previous<T, F>(
        self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'scope>>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), f, error_handler)
    }

    #[doc(hidden)]
    pub fn effect_with_previous_from<T, F>(
        self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'scope>>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        let effect = self
            .inner
            .effect_with_previous_from(inputs, f, error_handler)
            .map_err(map_effect_init_error)?;
        Ok(Effect::from_inner(effect))
    }

    pub fn watch<S, C>(
        self,
        source: S,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'scope>>
    where
        S: ReactiveSource<'scope>,
        S::Value: Sized + Clone + PartialEq + RxData + 'scope,
        C: FnMut(&S::Value, Option<&S::Value>) -> SilexResult<()> + 'scope,
    {
        self.watch_with_options(source, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_with_options<S, C>(
        self,
        source: S,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
        options: WatchOptions,
    ) -> SilexResult<Effect<'scope>>
    where
        S: ReactiveSource<'scope>,
        S::Value: Sized + Clone + PartialEq + RxData + 'scope,
        C: FnMut(&S::Value, Option<&S::Value>) -> SilexResult<()> + 'scope,
    {
        let plan = source.into_promotion_plan();
        let inputs = plan.inputs();
        self.try_validate_inputs(&inputs)?;
        let source = plan.materialize_unchecked(self);
        self.watch_getter_from(
            inputs,
            move || source.try_get().map_err(SilexError::from),
            callback,
            error_handler,
            options,
        )
    }

    pub fn watch_getter<T, G, C>(
        self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'scope>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> SilexResult<T> + 'scope,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'scope,
    {
        self.watch_getter_with_options(getter, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, G, C>(
        self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
        options: WatchOptions,
    ) -> SilexResult<Effect<'scope>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> SilexResult<T> + 'scope,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'scope,
    {
        self.watch_getter_from(
            RuntimeInputs::new(),
            getter,
            callback,
            error_handler,
            options,
        )
    }

    #[doc(hidden)]
    pub fn watch_getter_from<T, G, C>(
        self,
        inputs: RuntimeInputs,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
        options: WatchOptions,
    ) -> SilexResult<Effect<'scope>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> SilexResult<T> + 'scope,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'scope,
    {
        self.inner
            .watch_getter_from(inputs, getter, callback, error_handler, options)
            .map_err(map_effect_init_error)
            .map(Effect::from_inner)
    }

    pub fn stored<T: 'scope>(self, value: T) -> StoredValue<'scope, T> {
        StoredValue::from_inner(self.inner.stored(value), self)
    }

    pub fn callback<T, F>(self, callback: F) -> SilexResult<Callback<'scope, T>>
    where
        T: 'scope,
        F: FnMut(T) -> SilexResult<()> + 'scope,
    {
        self.inner
            .callback(callback)
            .map(Callback::from_inner)
            .map_err(SilexError::from)
    }

    pub fn node_ref<T: 'scope>(self) -> NodeRef<'scope, T> {
        NodeRef::from_inner(self.inner.node_ref())
    }

    /// Create a one-shot completion destination.
    ///
    /// Use [`crate::unwind_safe`] for callbacks that capture interior-mutable
    /// state.
    pub fn completion_once<T, F>(self, callback: F) -> CompletionOnce<T>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'scope,
    {
        self.inner.completion_once(callback)
    }

    /// Create a reusable completion destination.
    ///
    /// Use [`crate::unwind_safe`] for callbacks that capture interior-mutable
    /// state.
    pub fn completion_sender<T, F>(self, callback: F) -> CompletionSender<T>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'scope,
    {
        self.inner.completion_sender(callback)
    }

    /// Spawn a task owned by this persistent scope or the currently running computation.
    pub fn spawn_scoped<F>(self, future: F, error_handler: ErrorReporter<'scope>) -> TaskHandle
    where
        F: Future<Output = ()> + 'static,
    {
        if !self.is_active() {
            return TaskHandle::inactive();
        }
        let (task, cancel) = task::start(future);
        self.on_cleanup(
            move || {
                cancel();
                Ok::<(), SilexError>(())
            },
            error_handler,
        )
        .unwrap_or_else(|error| panic!("注册 scoped task cleanup 失败: {error}"));
        task
    }

    /// Promote a source after validating its complete opaque input set.
    ///
    /// Plan materialization is the only step allowed to register target
    /// nodes, so a foreign input fails before any target mutation.
    pub fn try_promote<T>(self, value: T) -> SilexResult<Rx<'scope, T::Value>>
    where
        T: ReactiveSource<'scope>,
        T::Value: Sized + RxData + 'scope,
    {
        value
            .into_promotion_plan()
            .materialize(self)
            .map_err(SilexError::from)
    }

    pub fn promote<T>(self, value: T) -> Rx<'scope, T::Value>
    where
        T: ReactiveSource<'scope>,
        T::Value: Sized + RxData + 'scope,
    {
        self.try_promote(value)
            .unwrap_or_else(|error| panic!("reactive promotion failed: {error}"))
    }

    #[doc(hidden)]
    pub fn try_validate_inputs(self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.inner
            .try_validate_inputs(inputs)
            .map_err(SilexError::from)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(self) -> RuntimeSnapshot {
        self.inner.runtime_snapshot()
    }

    pub(crate) fn assert_inputs(self, inputs: &RuntimeInputs) {
        if let Err(error) = self.try_validate_inputs(inputs) {
            panic!("reactive input validation failed: {error}");
        }
    }

    pub fn constant<T: 'scope>(self, value: T) -> Rx<'scope, T> {
        let stored = self.stored(value);
        Rx::from_stored(stored)
    }

    pub fn child<R>(self, f: impl for<'child> FnOnce(Scope<'child>) -> R) -> R {
        self.inner.child(|scope| f(Scope { inner: scope }))
    }

    pub fn untrack<R>(self, f: impl FnOnce() -> R) -> R {
        self.inner.untrack(f)
    }

    pub fn batch<R>(self, f: impl FnOnce() -> R) -> R {
        self.inner.batch(f)
    }

    /// Register a cleanup callback for this scope or the current computation.
    ///
    /// During final disposal of the owning scope, pending cleanup callbacks run
    /// before their nodes and payloads are dropped. A callback may synchronously
    /// read or update a [`StoredValue`] from the same scope during that window,
    /// even though [`Scope::is_active`] is already `false`. Other scope APIs
    /// remain unavailable. This guarantee applies only to final scope disposal,
    /// not to effect reruns or single-node stops.
    pub fn on_cleanup<F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<()>
    where
        F: FnOnce() -> SilexResult<()> + 'scope,
    {
        self.inner
            .on_cleanup(f, error_handler)
            .map_err(SilexError::from)
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
        self.try_child()
            .unwrap_or_else(|error| panic!("创建 owned child scope 失败: {error}"))
    }

    pub fn try_child(&self) -> SilexResult<Self> {
        self.inner
            .try_child()
            .map(|inner| Self { inner })
            .map_err(SilexError::from)
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.inner
            .try_validate_inputs(inputs)
            .map_err(SilexError::from)
    }

    /// Register and immediately run an owner-bound effect without extra
    /// framework-declared inputs.
    pub fn effect<F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'_>>
    where
        F: FnMut() -> SilexResult<()> + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f, error_handler)
    }

    pub fn effect_from<F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'_>>
    where
        F: FnMut() -> SilexResult<()> + 'scope,
    {
        self.inner
            .effect_from(inputs, f, error_handler)
            .map(Effect::from_inner)
            .map_err(map_effect_init_error)
    }

    pub fn effect_with_previous<T, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'_>>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), f, error_handler)
    }

    #[doc(hidden)]
    pub fn effect_with_previous_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'_>>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.inner
            .effect_with_previous_from(inputs, f, error_handler)
            .map(Effect::from_inner)
            .map_err(map_effect_init_error)
    }

    pub fn watch_getter<T, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Effect<'_>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> SilexResult<T> + 'scope,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'scope,
    {
        self.watch_getter_with_options(getter, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
        options: WatchOptions,
    ) -> SilexResult<Effect<'_>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> SilexResult<T> + 'scope,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'scope,
    {
        self.watch_getter_from(
            RuntimeInputs::new(),
            getter,
            callback,
            error_handler,
            options,
        )
    }

    #[doc(hidden)]
    pub fn watch_getter_from<T, G, C>(
        &self,
        inputs: RuntimeInputs,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, SilexError>,
        options: WatchOptions,
    ) -> SilexResult<Effect<'_>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> SilexResult<T> + 'scope,
        C: FnMut(&T, Option<&T>) -> SilexResult<()> + 'scope,
    {
        self.inner
            .watch_getter_from(inputs, getter, callback, error_handler, options)
            .map(Effect::from_inner)
            .map_err(map_effect_init_error)
    }

    /// Register a cleanup callback for this persistent owner.
    ///
    /// During final owner disposal, a callback may synchronously read or update
    /// a same-scope [`StoredValue`] before the payload is dropped. The owner is
    /// still inactive for ordinary APIs, and this guarantee does not apply to
    /// effect reruns or single-node stops.
    pub fn on_cleanup<F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<()>
    where
        F: FnOnce() -> SilexResult<()> + 'scope,
    {
        self.inner
            .on_cleanup(f, error_handler)
            .map_err(SilexError::from)
    }

    /// Create a one-shot completion destination.
    ///
    /// Use [`crate::unwind_safe`] for callbacks that capture interior-mutable
    /// state.
    pub fn completion_once<T, F>(&self, callback: F) -> CompletionOnce<T>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'scope,
    {
        self.inner.completion_once(callback)
    }

    /// Create a reusable completion destination.
    ///
    /// Use [`crate::unwind_safe`] for callbacks that capture interior-mutable
    /// state.
    pub fn completion_sender<T, F>(&self, callback: F) -> CompletionSender<T>
    where
        T: 'static,
        F: FnMut(T) -> SilexResult<()> + UnwindSafe + 'scope,
    {
        self.inner.completion_sender(callback)
    }

    /// Spawn a task owned by this persistent scope or the currently running computation.
    pub fn spawn_scoped<F>(&self, future: F, error_handler: ErrorReporter<'scope>) -> TaskHandle
    where
        F: Future<Output = ()> + 'static,
    {
        if !self.is_active() {
            return TaskHandle::inactive();
        }
        let (task, cancel) = task::start(future);
        self.on_cleanup(
            move || {
                cancel();
                Ok::<(), SilexError>(())
            },
            error_handler,
        )
        .unwrap_or_else(|error| panic!("注册 owned task cleanup 失败: {error}"));
        task
    }

    pub fn dispose(&self) {
        self.inner.dispose();
    }
}
