//! Owner-backed root scope and handle implementation.

use crate::{
    CompletionToken, ReactiveError, ReactiveResult,
    child::Scope,
    handle::NodeKind,
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk, OnceThunk},
    },
    root::node::{
        OwnedHandle, RootCallback, RootDerived, RootEffect, RootMemo, RootNodeRef, RootReadSignal,
        RootSignal, RootStoredValue, RootWriteSignal,
    },
    runtime::{self, RuntimeInputs, ScopeState},
    scope::ScopeStorage,
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    marker::PhantomData,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::{Rc, Weak},
};

pub(crate) type RootStateRef = RefCell<ScopeState<'static>>;

pub(crate) struct RootState {
    pub(crate) storage: ScopeStorage,
    pub(crate) dispose_hooks: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl RootState {
    fn new() -> Self {
        let scheduler = runtime::GlobalScheduler::new();
        Self {
            storage: ScopeStorage::new(scheduler),
            dispose_hooks: RefCell::new(Vec::new()),
        }
    }

    fn state(&self) -> Rc<RootStateRef> {
        self.storage.state.clone()
    }

    fn dispose(&self) -> Option<Box<dyn std::any::Any + Send>> {
        let state = self.state();
        let scheduler = state.borrow().scheduler.clone();
        scheduler
            .borrow_mut()
            .deactivate_scope(self.storage.scope_id);

        let mut first_panic = None;
        {
            let observer_frame = runtime::ObserverFrame::push(scheduler.clone(), None);
            for hook in mem::take(&mut *self.dispose_hooks.borrow_mut()) {
                if let Err(panic) = catch_unwind(AssertUnwindSafe(hook))
                    && first_panic.is_none()
                {
                    first_panic = Some(panic);
                }
            }
            drop(observer_frame);
        }

        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| self.storage.dispose_untracked()))
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
        scheduler.borrow_mut().clear_queue();
        first_panic
    }
}

/// A cleanup failure returned by an explicit root disposal.
pub struct CleanupError {
    panic: Box<dyn std::any::Any + Send>,
}

impl fmt::Debug for CleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CleanupError").finish_non_exhaustive()
    }
}

impl CleanupError {
    fn new(panic: Box<dyn std::any::Any + Send>) -> Self {
        Self { panic }
    }

    pub(crate) fn report_during_unwind(self) {
        eprintln!("silex_reactivity: root cleanup panicked while handling another panic");
        drop(self.panic);
    }

    fn resume(self) -> ! {
        resume_unwind(self.panic)
    }
}

/// Owns one long-lived root scope.
pub struct RootHandle {
    owner: Option<Rc<RootState>>,
    runtime_slot: Rc<Cell<bool>>,
}

impl RootHandle {
    pub(crate) fn new(runtime_slot: Rc<Cell<bool>>) -> Self {
        Self {
            owner: Some(Rc::new(RootState::new())),
            runtime_slot,
        }
    }

    /// Return the capability used to register root-owned nodes and cleanup.
    pub fn scope(&self) -> RootScope {
        RootScope {
            owner: self.owner.as_ref().map_or_else(Weak::new, Rc::downgrade),
        }
    }

    /// Dispose the root exactly once.
    pub fn dispose(&mut self) -> Result<(), CleanupError> {
        let Some(owner) = self.owner.take() else {
            self.runtime_slot.set(false);
            return Ok(());
        };
        self.runtime_slot.set(false);
        match owner.dispose() {
            Some(panic) => Err(CleanupError::new(panic)),
            None => Ok(()),
        }
    }

    pub fn is_active(&self) -> bool {
        self.owner.is_some()
    }
}

impl Drop for RootHandle {
    fn drop(&mut self) {
        if let Err(error) = self.dispose() {
            error.resume();
        }
    }
}

/// Capability for creating nodes that may outlive the `Runtime::run` callback.
///
/// Every value and callback registered through this type is required to be
/// `'static`. The capability itself only keeps a weak owner reference, so it
/// becomes inert as soon as [`RootHandle`] is disposed.
#[derive(Clone)]
pub struct RootScope {
    owner: Weak<RootState>,
}

impl RootScope {
    fn state(&self) -> ReactiveResult<Rc<RootStateRef>> {
        let owner = self.owner.upgrade().ok_or(ReactiveError::NoSuchNode)?;
        let state = owner.state();
        let active = state
            .borrow()
            .scheduler
            .borrow()
            .is_scope_active(owner.storage.scope_id);
        active.then_some(state).ok_or(ReactiveError::NoSuchNode)
    }

    fn handle<K: NodeKind>(&self, state: &Rc<RootStateRef>, raw: RawId) -> OwnedHandle<K> {
        let scope_id = state.borrow().scope_id;
        OwnedHandle::new(Rc::downgrade(state), scope_id, raw)
    }

    pub fn is_active(&self) -> bool {
        self.state().is_ok()
    }

    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> ReactiveResult<()> {
        let state = self.state()?;
        runtime::validate_inputs(&state, inputs)
    }

    pub fn signal<T: 'static>(&self, value: T) -> (RootReadSignal<T>, RootWriteSignal<T>) {
        let state = self.state().expect("创建 root signal 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 signal 创建期间被借用")
            .create_signal(AnyValue::new(value));
        let handle = self.handle(&state, raw);
        (
            RootReadSignal::new(handle.clone()),
            RootWriteSignal::new(handle),
        )
    }

    pub fn rw_signal<T: 'static>(&self, value: T) -> RootSignal<T> {
        let (read, write) = self.signal(value);
        RootSignal::new(read, write)
    }

    pub fn effect<F>(&self, f: F) -> RootEffect
    where
        F: FnMut() + 'static,
    {
        self.effect_from(RuntimeInputs::new(), f)
    }

    /// Create a root effect after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> RootEffect
    where
        F: FnMut() + 'static,
    {
        self.try_effect_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 root effect 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> ReactiveResult<RootEffect>
    where
        F: FnMut() + 'static,
    {
        let state = self.state()?;
        let raw = runtime::create_effect(&state, inputs, f)?;
        let handle = self.handle(&state, raw);
        Ok(RootEffect::new(handle))
    }

    pub fn memo<T, F>(&self, f: F) -> RootMemo<T>
    where
        T: PartialEq + 'static,
        F: FnMut(Option<&T>) -> T + 'static,
    {
        self.memo_from(RuntimeInputs::new(), f)
    }

    /// Create a root memo after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn memo_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> RootMemo<T>
    where
        T: PartialEq + 'static,
        F: FnMut(Option<&T>) -> T + 'static,
    {
        self.try_memo_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 root memo 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_memo_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> ReactiveResult<RootMemo<T>>
    where
        T: PartialEq + 'static,
        F: FnMut(Option<&T>) -> T + 'static,
    {
        let state = self.state()?;
        let raw = runtime::create_memo(&state, inputs, f)?;
        let handle = self.handle(&state, raw);
        Ok(RootMemo::new(handle))
    }

    pub fn derived<T, F>(&self, f: F) -> RootDerived<T>
    where
        T: 'static,
        F: FnMut() -> T + 'static,
    {
        self.derived_from(RuntimeInputs::new(), f)
    }

    /// Create a root derived value after validating all declared reactive
    /// inputs.
    #[doc(hidden)]
    pub fn derived_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> RootDerived<T>
    where
        T: 'static,
        F: FnMut() -> T + 'static,
    {
        self.try_derived_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 root derived 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_derived_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
    ) -> ReactiveResult<RootDerived<T>>
    where
        T: 'static,
        F: FnMut() -> T + 'static,
    {
        let state = self.state()?;
        let raw = runtime::create_derived(&state, inputs, f)?;
        let handle = self.handle(&state, raw);
        Ok(RootDerived::new(handle))
    }

    pub fn stored<T: 'static>(&self, value: T) -> RootStoredValue<T> {
        let state = self
            .state()
            .expect("创建 root stored value 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 stored value 创建期间被借用")
            .create_stored(AnyValue::new(value));
        RootStoredValue::new(self.handle(&state, raw))
    }

    pub fn callback<T, F>(&self, callback: F) -> RootCallback<T>
    where
        T: 'static,
        F: FnMut(T) + 'static,
    {
        let state = self.state().expect("创建 root callback 时 owner 已结束");
        let thunk = CallbackThunk::new_typed(callback);
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 callback 创建期间被借用")
            .create_callback(thunk);
        RootCallback::new(self.handle(&state, raw))
    }

    pub fn node_ref<T: 'static>(&self) -> RootNodeRef<T> {
        let state = self.state().expect("创建 root node ref 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 node ref 创建期间被借用")
            .create_node_ref(AnyValue::new(Option::<T>::None));
        RootNodeRef::new(self.handle(&state, raw))
    }

    pub fn completion<T, F>(&self, callback: F) -> CompletionToken<T>
    where
        T: 'static,
        F: FnMut(T) + 'static,
    {
        let Ok(state) = self.state() else {
            return CompletionToken::inactive();
        };
        let thunk = CallbackThunk::new_typed(callback);
        let (raw, scope_id, weak) = {
            let mut state_ref = state
                .try_borrow_mut()
                .expect("root state 在 completion 创建期间被借用");
            let raw = state_ref.create_callback(thunk);
            (raw, state_ref.scope_id, Rc::downgrade(&state))
        };
        CompletionToken::new(weak, scope_id, raw)
    }

    pub fn on_cleanup<F>(&self, cleanup: F)
    where
        F: FnOnce() + 'static,
    {
        let state = self.state().expect("注册 root cleanup 时 owner 已结束");
        state
            .try_borrow_mut()
            .expect("root state 在 cleanup 注册期间被借用")
            .register_cleanup(OnceThunk::new(cleanup));
    }

    /// Register a host-resource cancellation hook owned by this root.
    pub fn on_dispose<F>(&self, hook: F)
    where
        F: FnOnce() + 'static,
    {
        let owner = self
            .owner
            .upgrade()
            .expect("注册 root dispose hook 时 owner 已结束");
        if !self.is_active() {
            return;
        }
        owner.dispose_hooks.borrow_mut().push(Box::new(hook));
    }

    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        let state = self.state().expect("root untrack 时 owner 已结束");
        runtime::with_untracked(&state, f)
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        let state = self.state().expect("root batch 时 owner 已结束");
        runtime::with_batch(&state, f)
    }

    /// Create a lexical child scope under the long-lived root.
    pub fn child<R>(&self, f: impl for<'scope> FnOnce(&'scope Scope<'scope>) -> R) -> R {
        let state = self.state().expect("创建 root child scope 时 owner 已结束");
        let scheduler = state.borrow().scheduler.clone();
        let storage = ScopeStorage::new(scheduler.clone());
        let child = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let observer_frame = runtime::ObserverFrame::push_child(scheduler, storage.scope_id);
        let result = catch_unwind(AssertUnwindSafe(|| f(&child)));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| storage.dispose_untracked()));
        drop(observer_frame);
        match (result, dispose_result) {
            (Ok(value), Ok(())) => value,
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
        }
    }

    /// Create a persistent owner backed by the root scheduler.
    pub fn owned_scope(&self) -> crate::OwnedScope<'static> {
        let state = self.state().expect("创建 owned scope 时 root owner 已结束");
        let scheduler = state.borrow().scheduler.clone();
        crate::OwnedScope::new_for_scheduler(scheduler)
    }
}
