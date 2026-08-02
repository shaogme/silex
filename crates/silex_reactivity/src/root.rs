//! Long-lived, owner-backed root scopes.

use crate::{
    CompletionToken, ReactiveError, ReactiveResult,
    handle::{NodeKind, kind},
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk, EffectThunk, MemoThunk, OnceThunk},
    },
    runtime::{self, ScopeId, ScopeState},
    scope::{Scope, ScopeFrame},
};
use std::{
    cell::{Cell, RefCell},
    fmt,
    marker::PhantomData,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::{Rc, Weak},
};

type RootStateRef = RefCell<ScopeState<'static>>;

struct RootState {
    frame: ScopeFrame<'static>,
    dispose_hooks: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl RootState {
    fn new() -> Self {
        let scheduler = runtime::GlobalScheduler::new();
        Self {
            frame: ScopeFrame::new(scheduler),
            dispose_hooks: RefCell::new(Vec::new()),
        }
    }

    fn state(&self) -> Rc<RootStateRef> {
        self.frame.state.clone()
    }

    fn dispose(&self) -> Option<Box<dyn std::any::Any + Send>> {
        let state = self.state();
        let scheduler = state.borrow().scheduler.clone();
        scheduler.borrow_mut().deactivate_scope(self.frame.scope_id);

        let mut first_panic = None;
        for hook in mem::take(&mut *self.dispose_hooks.borrow_mut()) {
            if let Err(panic) = catch_unwind(AssertUnwindSafe(hook))
                && first_panic.is_none()
            {
                first_panic = Some(panic);
            }
        }

        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| self.frame.dispose()))
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
            .is_scope_active(owner.frame.scope_id);
        active.then_some(state).ok_or(ReactiveError::NoSuchNode)
    }

    fn handle<K: NodeKind>(&self, state: &Rc<RootStateRef>, raw: RawId) -> OwnedHandle<K> {
        let scope_id = state.borrow().scope_id;
        OwnedHandle {
            state: Rc::downgrade(state),
            scope_id,
            raw,
            marker: PhantomData,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state().is_ok()
    }

    pub fn signal<T: 'static>(&self, value: T) -> (RootReadSignal<T>, RootWriteSignal<T>) {
        let state = self.state().expect("创建 root signal 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 signal 创建期间被借用")
            .create_signal(AnyValue::new(value));
        let handle = self.handle(&state, raw);
        (
            RootReadSignal {
                handle: handle.clone(),
                marker: PhantomData,
            },
            RootWriteSignal {
                handle,
                marker: PhantomData,
            },
        )
    }

    pub fn rw_signal<T: 'static>(&self, value: T) -> RootSignal<T> {
        let (read, write) = self.signal(value);
        RootSignal { read, write }
    }

    pub fn effect<F>(&self, f: F) -> RootEffect
    where
        F: FnMut() + 'static,
    {
        let state = self.state().expect("创建 root effect 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 effect 创建期间被借用")
            .create_effect(EffectThunk::new(f));
        let handle = self.handle(&state, raw);
        runtime::run_initial(&state, raw);
        RootEffect { handle }
    }

    pub fn memo<T, F>(&self, f: F) -> RootMemo<T>
    where
        T: PartialEq + 'static,
        F: FnMut(Option<&T>) -> T + 'static,
    {
        let state = self.state().expect("创建 root memo 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 memo 创建期间被借用")
            .create_memo(MemoThunk::new::<T, F>(f), false);
        let handle = self.handle(&state, raw);
        runtime::run_initial(&state, raw);
        RootMemo {
            handle,
            marker: PhantomData,
        }
    }

    pub fn derived<T, F>(&self, f: F) -> RootDerived<T>
    where
        T: 'static,
        F: FnMut() -> T + 'static,
    {
        let state = self.state().expect("创建 root derived 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 derived 创建期间被借用")
            .create_memo(MemoThunk::new_derived::<T, F>(f), true);
        let handle = self.handle(&state, raw);
        runtime::run_initial(&state, raw);
        RootDerived {
            handle,
            marker: PhantomData,
        }
    }

    pub fn stored<T: 'static>(&self, value: T) -> RootStoredValue<T> {
        let state = self
            .state()
            .expect("创建 root stored value 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 stored value 创建期间被借用")
            .create_stored(AnyValue::new(value));
        RootStoredValue {
            handle: self.handle(&state, raw),
            marker: PhantomData,
        }
    }

    pub fn callback<T, F>(&self, callback: F) -> RootCallback<T>
    where
        T: 'static,
        F: FnMut(T) + 'static,
    {
        let state = self.state().expect("创建 root callback 时 owner 已结束");
        let mut callback = callback;
        let thunk = CallbackThunk::new(move |value: AnyValue<'static>| {
            if let Some(value) = unsafe { value.downcast::<T>() } {
                callback(value);
            }
        });
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 callback 创建期间被借用")
            .create_callback(thunk);
        RootCallback {
            handle: self.handle(&state, raw),
            marker: PhantomData,
        }
    }

    pub fn node_ref<T: 'static>(&self) -> RootNodeRef<T> {
        let state = self.state().expect("创建 root node ref 时 owner 已结束");
        let raw = state
            .try_borrow_mut()
            .expect("root state 在 node ref 创建期间被借用")
            .create_node_ref(AnyValue::new(Option::<T>::None));
        RootNodeRef {
            handle: self.handle(&state, raw),
            marker: PhantomData,
        }
    }

    pub fn completion<T, F>(&self, callback: F) -> CompletionToken<T>
    where
        T: 'static,
        F: FnMut(T) + 'static,
    {
        let state = self.state().expect("创建 root completion 时 owner 已结束");
        let mut callback = callback;
        let thunk = CallbackThunk::new(move |value: AnyValue<'static>| {
            if let Some(value) = unsafe { value.downcast::<T>() } {
                callback(value);
            }
        });
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
    pub fn scope<R>(&self, f: impl for<'s1, 's2, 's3> FnOnce(&'s1 Scope<'s2, 's3>) -> R) -> R {
        let state = self.state().expect("创建 root child scope 时 owner 已结束");
        let scheduler = state.borrow().scheduler.clone();
        let frame = ScopeFrame::new(scheduler);
        let child = Scope {
            frame: &frame,
            _marker: PhantomData,
        };
        let result = catch_unwind(AssertUnwindSafe(|| f(&child)));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| frame.dispose()));
        match (result, dispose_result) {
            (Ok(value), Ok(())) => value,
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
        }
    }
}

struct OwnedHandle<K: NodeKind> {
    state: Weak<RootStateRef>,
    scope_id: ScopeId,
    raw: RawId,
    marker: PhantomData<fn() -> K>,
}

impl<K: NodeKind> Clone for OwnedHandle<K> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            scope_id: self.scope_id,
            raw: self.raw,
            marker: PhantomData,
        }
    }
}

impl<K: NodeKind> OwnedHandle<K> {
    fn state(&self) -> ReactiveResult<Rc<RootStateRef>> {
        let state = self.state.upgrade().ok_or(ReactiveError::NoSuchNode)?;
        let active = state
            .borrow()
            .scheduler
            .borrow()
            .is_scope_active(self.scope_id);
        active.then_some(state).ok_or(ReactiveError::NoSuchNode)
    }

    fn raw(&self) -> RawId {
        self.raw
    }

    fn is_alive(&self) -> bool {
        self.state()
            .ok()
            .is_some_and(|state| state.borrow().node_kind(self.raw) == Some(K::TAG))
    }
}

pub struct RootReadSignal<T> {
    handle: OwnedHandle<kind::Signal>,
    marker: PhantomData<fn() -> T>,
}

pub struct RootWriteSignal<T> {
    handle: OwnedHandle<kind::Signal>,
    marker: PhantomData<fn(T)>,
}

pub struct RootSignal<T> {
    read: RootReadSignal<T>,
    write: RootWriteSignal<T>,
}

pub struct RootEffect {
    handle: OwnedHandle<kind::Effect>,
}

pub struct RootMemo<T> {
    handle: OwnedHandle<kind::Memo>,
    marker: PhantomData<fn() -> T>,
}

pub struct RootDerived<T> {
    handle: OwnedHandle<kind::Derived>,
    marker: PhantomData<fn() -> T>,
}

pub struct RootStoredValue<T> {
    handle: OwnedHandle<kind::Stored>,
    marker: PhantomData<fn() -> T>,
}

pub struct RootCallback<T> {
    handle: OwnedHandle<kind::Callback>,
    marker: PhantomData<fn(T)>,
}

pub struct RootNodeRef<T> {
    handle: OwnedHandle<kind::NodeRef>,
    marker: PhantomData<fn() -> T>,
}

macro_rules! impl_root_clone {
    ($($ty:ident $(<$generic:ident>)?;)+) => {
        $(
            impl<$($generic: 'static,)? > Clone for $ty$(<$generic>)? {
                fn clone(&self) -> Self {
                    Self {
                        handle: self.handle.clone(),
                        marker: PhantomData,
                    }
                }
            }
        )+
    };
}

impl_root_clone! {
    RootReadSignal<T>;
    RootWriteSignal<T>;
    RootMemo<T>;
    RootDerived<T>;
    RootStoredValue<T>;
    RootCallback<T>;
    RootNodeRef<T>;
}

impl Clone for RootEffect {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
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

macro_rules! impl_root_read {
    ($ty:ident, $kind:ident) => {
        impl<T: 'static> $ty<T> {
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
                self.try_get().expect("读取 root reactive node 失败")
            }

            pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
                let state = self.handle.state().expect("读取 root reactive node 失败");
                runtime::with_signal(&state, self.handle.raw(), true, |value| {
                    unsafe { value.downcast_ref::<T>() }
                        .map(f)
                        .ok_or(ReactiveError::TypeMismatch)
                })
                .expect("读取 root reactive node 失败")
                .expect("读取 root reactive node 类型不匹配")
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
        }
    };
}

impl_root_read!(RootReadSignal, Signal);
impl_root_read!(RootMemo, Memo);
impl_root_read!(RootDerived, Derived);

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
}

impl RootEffect {
    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
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

    pub fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}
