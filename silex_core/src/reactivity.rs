use std::cell::{Cell, RefCell};
use std::future::Future;
use std::marker::PhantomData;
use std::panic::Location;
use std::rc::Rc;

pub use silex_reactivity::NodeId;
pub use silex_reactivity::{create_scope, dispose, on_cleanup, provide_context, use_context};

use crate::SilexError;
use crate::traits::*;

// --- Accessor Trait ---

pub trait Accessor<T> {
    fn value(&self) -> T;
}

impl<F, T> Accessor<T> for F
where
    F: Fn() -> T,
{
    fn value(&self) -> T {
        self()
    }
}

impl<T: Clone + 'static> Accessor<T> for Signal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + 'static> Accessor<T> for ReadSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + 'static> Accessor<T> for RwSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + PartialEq + 'static> Accessor<T> for Memo<T> {
    fn value(&self) -> T {
        self.get()
    }
}

// --- Signal 信号 Enum ---

#[derive(Debug)]
pub enum Signal<T: 'static> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

impl<T: Clone + 'static> Signal<T> {
    pub fn derive(f: impl Fn() -> T + 'static) -> Self {
        let id = silex_reactivity::register_derived(move || Box::new(f()));
        Signal::Derived(id, PhantomData)
    }
}

impl<T> DefinedAt for Signal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for Signal<T> {
    fn is_disposed(&self) -> bool {
        match self {
            Signal::Read(s) => s.is_disposed(),
            Signal::Derived(id, _) => !silex_reactivity::is_signal_valid(*id),
        }
    }
}

impl<T: 'static> Track for Signal<T> {
    fn track(&self) {
        match self {
            Signal::Read(s) => s.track(),
            Signal::Derived(id, _) => {
                // Run the derived function to track its dependencies
                let _ = silex_reactivity::run_derived::<T>(*id);
            }
        }
    }
}

impl<T: 'static> WithUntracked for Signal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        match self {
            Signal::Read(s) => s.try_with_untracked(fun),
            Signal::Derived(id, _) => {
                let val = untrack(|| silex_reactivity::run_derived::<T>(*id))?;
                Some(fun(&val))
            }
        }
    }
}

impl<T: Clone + 'static> From<T> for Signal<T> {
    fn from(value: T) -> Self {
        let (read, _) = signal(value);
        Signal::Read(read)
    }
}

impl From<&str> for Signal<String> {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl<T: 'static> From<ReadSignal<T>> for Signal<T> {
    fn from(s: ReadSignal<T>) -> Self {
        Signal::Read(s)
    }
}

impl<T: 'static> From<RwSignal<T>> for Signal<T> {
    fn from(s: RwSignal<T>) -> Self {
        Signal::Read(s.read)
    }
}

impl<T: 'static> From<Memo<T>> for Signal<T> {
    fn from(m: Memo<T>) -> Self {
        Signal::Read(ReadSignal {
            id: m.id,
            marker: PhantomData,
        })
    }
}

// --- ReadSignal ---

pub struct ReadSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for ReadSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReadSignal({:?})", self.id)
    }
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ReadSignal<T> {}

impl<T> DefinedAt for ReadSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for ReadSignal<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

impl<T> Track for ReadSignal<T> {
    fn track(&self) {
        silex_reactivity::track_signal(self.id);
    }
}

impl<T: 'static> WithUntracked for ReadSignal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_signal_untracked(self.id, fun)
    }
}

impl<T: 'static + Clone> ReadSignal<T> {
    pub fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        Memo::new(move |_| f(self.get()))
    }
}

// Fluent API Extensions for ReadSignal
impl<T: Clone + 'static + PartialEq> ReadSignal<T> {
    pub fn eq<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() == other)
    }

    pub fn ne<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() != other)
    }
}

impl<T: Clone + 'static + PartialOrd> ReadSignal<T> {
    pub fn gt<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() > other)
    }

    pub fn lt<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() < other)
    }

    pub fn ge<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() >= other)
    }

    pub fn le<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() <= other)
    }
}

// --- WriteSignal ---

pub struct WriteSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for WriteSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WriteSignal({:?})", self.id)
    }
}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for WriteSignal<T> {}

impl<T> DefinedAt for WriteSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for WriteSignal<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

impl<T> Notify for WriteSignal<T> {
    fn notify(&self) {
        silex_reactivity::notify_signal(self.id);
    }
}

impl<T: 'static> UpdateUntracked for WriteSignal<T> {
    type Value = T;

    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_update_signal_silent(self.id, fun)
    }
}

impl<T: 'static> WriteSignal<T> {
    pub fn setter(self, value: T) -> impl Fn()
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    pub fn updater<F>(self, f: F) -> impl Fn()
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

// --- RwSignal ---

pub struct RwSignal<T: 'static> {
    pub read: ReadSignal<T>,
    pub write: WriteSignal<T>,
}

impl<T> Clone for RwSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for RwSignal<T> {}

impl<T: 'static> RwSignal<T> {
    pub fn new(value: T) -> Self {
        let (read, write) = signal(value);
        RwSignal { read, write }
    }

    pub fn read_signal(&self) -> ReadSignal<T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<T> {
        self.write
    }

    pub fn split(&self) -> (ReadSignal<T>, WriteSignal<T>) {
        (self.read, self.write)
    }

    pub fn from_parts(read: ReadSignal<T>, write: WriteSignal<T>) -> Self {
        Self { read, write }
    }

    pub fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
        T: Clone,
    {
        self.read.map(f)
    }

    pub fn setter(self, value: T) -> impl Fn()
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    pub fn updater<F>(self, f: F) -> impl Fn()
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

impl<T: 'static> DefinedAt for RwSignal<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T: 'static> IsDisposed for RwSignal<T> {
    fn is_disposed(&self) -> bool {
        self.read.is_disposed()
    }
}

impl<T: 'static> Track for RwSignal<T> {
    fn track(&self) {
        self.read.track();
    }
}

impl<T: 'static> Notify for RwSignal<T> {
    fn notify(&self) {
        self.write.notify();
    }
}

impl<T: 'static> WithUntracked for RwSignal<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.read.try_with_untracked(fun)
    }
}

impl<T: 'static> UpdateUntracked for RwSignal<T> {
    type Value = T;

    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        self.write.try_update_untracked(fun)
    }
}

// --- Memo ---

pub struct Memo<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for Memo<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Memo({:?})", self.id)
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Memo<T> {}

impl<T: Clone + PartialEq + 'static> Memo<T> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&T>) -> T + 'static,
    {
        let id = silex_reactivity::memo(f);
        Memo {
            id,
            marker: PhantomData,
        }
    }
}

impl<T> DefinedAt for Memo<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T> IsDisposed for Memo<T> {
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_signal_valid(self.id)
    }
}

impl<T> Track for Memo<T> {
    fn track(&self) {
        silex_reactivity::track_signal(self.id);
    }
}

impl<T: 'static> WithUntracked for Memo<T> {
    type Value = T;

    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_signal_untracked(self.id, fun)
    }
}

// --- Resource ---

pub struct Resource<T: 'static, E: 'static = SilexError> {
    pub data: ReadSignal<Option<T>>,
    pub error: ReadSignal<Option<E>>,
    pub loading: ReadSignal<bool>,
    trigger: WriteSignal<usize>,
}

impl<T, E> Clone for Resource<T, E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, E> Copy for Resource<T, E> {}

pub trait ResourceFetcher<S> {
    type Data;
    type Error;
    type Future: Future<Output = Result<Self::Data, Self::Error>>;

    fn fetch(&self, source: S) -> Self::Future;
}

impl<S, T, E, Fun, Fut> ResourceFetcher<S> for Fun
where
    Fun: Fn(S) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    type Data = T;
    type Error = E;
    type Future = Fut;

    fn fetch(&self, source: S) -> Self::Future {
        self(source)
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> Resource<T, E> {
    pub fn new<S, Fetcher>(source: impl Fn() -> S + 'static, fetcher: Fetcher) -> Self
    where
        S: PartialEq + Clone + 'static,
        Fetcher: ResourceFetcher<S, Data = T, Error = E> + 'static,
    {
        let (data, set_data) = signal(None);
        let (error, set_error) = signal(None);
        let (loading, set_loading) = signal(false);
        let (trigger, set_trigger) = signal(0);

        let alive = Rc::new(Cell::new(true));
        let alive_clone = alive.clone();
        on_cleanup(move || alive_clone.set(false));

        let request_id = Rc::new(Cell::new(0usize));

        Effect::new(move |_| {
            let source_val = source();
            let _ = trigger.get();

            let suspense_ctx = use_suspense_context();
            if let Some(ctx) = &suspense_ctx {
                ctx.increment();
            }
            set_loading.set(true);

            let current_id = request_id.get().wrapping_add(1);
            request_id.set(current_id);

            let fut = fetcher.fetch(source_val);
            let suspense_ctx = suspense_ctx.clone();

            let alive = alive.clone();
            let request_id = request_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let res = fut.await;

                if alive.get() && request_id.get() == current_id {
                    match res {
                        Ok(val) => {
                            set_data.set(Some(val));
                            set_error.set(None);
                        }
                        Err(e) => {
                            set_error.set(Some(e));
                        }
                    }
                    set_loading.set(false);
                }

                if let Some(ctx) = &suspense_ctx {
                    ctx.decrement();
                }
            });
        });

        Resource {
            data,
            error,
            loading,
            trigger: set_trigger,
        }
    }

    pub fn refetch(&self) {
        self.trigger.update(|n| *n = n.wrapping_add(1));
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> DefinedAt for Resource<T, E> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

// Resource implements Get to return Option<T>
impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> Get for Resource<T, E> {
    type Value = Option<T>;

    fn try_get(&self) -> Option<Self::Value> {
        if let Some(e) = self.error.get() {
            if let Some(ctx) = use_context::<crate::error::ErrorContext>() {
                let err_msg = format!("{:?}", e);
                (ctx.0)(crate::error::SilexError::Javascript(err_msg));
            }
        }
        self.data.try_get()
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> GetUntracked for Resource<T, E> {
    type Value = Option<T>;

    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.data.try_get_untracked()
    }
}

// --- StoredValue ---

pub struct StoredValue<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for StoredValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredValue({:?})", self.id)
    }
}

impl<T> Clone for StoredValue<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for StoredValue<T> {}

impl<T: 'static> StoredValue<T> {
    pub fn new(value: T) -> Self {
        let id = silex_reactivity::store_value(value);
        Self {
            id,
            marker: PhantomData,
        }
    }

    // Kept for backward compat or ease of use
    pub fn set_value(&self, value: T) {
        SetValue::set_value(self, value)
    }

    pub fn get_value(&self) -> T
    where
        T: Clone,
    {
        GetValue::get_value(self)
    }
}

impl<T> DefinedAt for StoredValue<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }
}

impl<T: 'static> WithValue for StoredValue<T> {
    type Value = T;

    fn try_with_value<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_stored_value(self.id, fun)
    }
}

impl<T: 'static> UpdateValue for StoredValue<T> {
    type Value = T;

    fn try_update_value<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_update_stored_value(self.id, fun)
    }
}

impl<T: 'static> SetValue for StoredValue<T> {
    type Value = T;

    fn try_set_value(&self, value: Self::Value) -> Option<Self::Value> {
        let value_wrapper = Rc::new(Cell::new(Some(value)));
        let value_in_closure = value_wrapper.clone();

        let res = self.try_update_value(move |v| {
            if let Some(new_val) = value_in_closure.take() {
                *v = new_val;
            }
        });

        if res.is_some() {
            None
        } else {
            value_wrapper.take()
        }
    }
}

// --- Global Functions ---

pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let id = silex_reactivity::signal(value);
    (
        ReadSignal {
            id,
            marker: PhantomData,
        },
        WriteSignal {
            id,
            marker: PhantomData,
        },
    )
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    silex_reactivity::untrack(f)
}

// --- Effect ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Effect {
    pub(crate) id: NodeId,
}

impl Effect {
    pub fn new<T, F>(f: F) -> Self
    where
        T: 'static,
        F: Fn(Option<T>) -> T + 'static,
    {
        let val = Rc::new(RefCell::new(None::<T>));
        let val_clone = val.clone();

        let id = silex_reactivity::effect(move || {
            let old = val_clone.borrow_mut().take();
            let new = f(old);
            *val_clone.borrow_mut() = Some(new);
        });
        Effect { id }
    }

    pub fn watch<W, T, C>(deps: W, callback: C, immediate: bool) -> Self
    where
        W: Fn() -> T + 'static,
        T: Clone + PartialEq + 'static,
        C: Fn(&T, Option<&T>, Option<()>) + 'static,
    {
        let first_run = Rc::new(Cell::new(true));
        let prev_deps = Rc::new(RefCell::new(None::<T>));

        Effect::new(move |_| {
            let new_val = deps();
            let mut p_borrow = prev_deps.borrow_mut();
            let old_val = p_borrow.clone();

            let is_first = first_run.get();
            if is_first {
                first_run.set(false);
                *p_borrow = Some(new_val.clone());
                if immediate {
                    callback(&new_val, old_val.as_ref(), None);
                }
            } else {
                if old_val.as_ref() != Some(&new_val) {
                    callback(&new_val, old_val.as_ref(), None);
                    *p_borrow = Some(new_val);
                }
            }
        })
    }
}

// --- Context ---

pub fn expect_context<T: Clone + 'static>() -> T {
    match use_context::<T>() {
        Some(v) => v,
        None => {
            let type_name = std::any::type_name::<T>();
            let msg = format!(
                "Expected context `{}` but none found. Did you forget to wrap your component in a Provider?",
                type_name
            );
            crate::log::console_error(&msg);
            panic!("{}", msg);
        }
    }
}

// --- Suspense ---

#[derive(Clone, Copy)]
pub struct SuspenseContext {
    pub count: ReadSignal<usize>,
    pub set_count: WriteSignal<usize>,
}

impl SuspenseContext {
    pub fn new() -> Self {
        let (count, set_count) = signal(0);
        Self { count, set_count }
    }

    pub fn increment(&self) {
        self.set_count.update(|c| *c += 1);
    }

    pub fn decrement(&self) {
        self.set_count.update(|c| {
            if *c > 0 {
                *c -= 1
            }
        });
    }
}

pub fn use_suspense_context() -> Option<SuspenseContext> {
    use_context::<SuspenseContext>()
}
