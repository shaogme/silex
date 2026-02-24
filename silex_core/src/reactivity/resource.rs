use std::cell::Cell;
use std::future::Future;
use std::panic::Location;
use std::rc::Rc;

use silex_reactivity::{on_cleanup, use_context};

use crate::SilexError;
use crate::reactivity::Memo;
use crate::traits::*;
use crate::{Rx, RxValue};
use std::marker::PhantomData;

use super::effect::Effect;
use super::signal::{ReadSignal, WriteSignal, signal};

// --- Resource ---

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceState<T, E> {
    /// Initial state, no data fetch has started yet.
    Idle,
    /// Loading initial data.
    Loading,
    /// Has data successfully.
    Ready(T),
    /// Has data, but is refreshing (Stale-While-Revalidate).
    Reloading(T),
    /// Failed to load data. Use `Resource::refetch` to retry.
    Error(E),
}

impl<T, E> ResourceState<T, E> {
    pub fn as_option(&self) -> Option<&T> {
        match self {
            Self::Ready(data) | Self::Reloading(data) => Some(data),
            _ => None,
        }
    }

    pub fn unwrap(self) -> T {
        match self {
            Self::Ready(data) | Self::Reloading(data) => data,
            _ => panic!("ResourceState::unwrap called on non-Ready/Reloading state"),
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading | Self::Reloading(_))
    }
}

pub struct Resource<T: 'static, E: 'static = SilexError> {
    pub state: ReadSignal<ResourceState<T, E>>,
    set_state: WriteSignal<ResourceState<T, E>>,
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
    pub fn new<S, Fetcher, R>(source: R, fetcher: Fetcher) -> Self
    where
        R: Read<Value = S> + 'static,
        for<'a> R::ReadOutput<'a>: std::ops::Deref<Target = S>,
        S: PartialEq + Clone + 'static,
        Fetcher: ResourceFetcher<S, Data = T, Error = E> + 'static,
    {
        // 默认状态为 Idle，直到第一次 Effect 执行变为 Loading
        let (state, set_state) = signal::<ResourceState<T, E>>(ResourceState::Idle);
        let (trigger, set_trigger) = signal(0);

        let alive = Rc::new(Cell::new(true));
        let alive_clone = alive.clone();
        on_cleanup(move || alive_clone.set(false));

        let request_id = Rc::new(Cell::new(0usize));

        Effect::new(move |_| {
            let source_val = source.get();
            let _ = trigger.get();

            let suspense_ctx = use_suspense_context();
            if let Some(ctx) = &suspense_ctx {
                ctx.increment();
            }

            // State transition logic:
            set_state.update(|s| {
                *s = match &*s {
                    // If we already have data (Ready or Reloading), switch to Reloading to preserve data
                    ResourceState::Ready(data) | ResourceState::Reloading(data) => {
                        ResourceState::Reloading(data.clone())
                    }
                    // Otherwise (Idle, Loading, Error), switch to Loading
                    _ => ResourceState::Loading,
                };
            });

            let current_id = request_id.get().wrapping_add(1);
            request_id.set(current_id);

            let fut = fetcher.fetch(source_val);

            let alive = alive.clone();
            let request_id = request_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let res = fut.await;

                if alive.get() && request_id.get() == current_id {
                    set_state.update(|s| {
                        *s = match res {
                            Ok(val) => ResourceState::Ready(val),
                            Err(e) => ResourceState::Error(e),
                        };
                    });
                }

                if let Some(ctx) = &suspense_ctx {
                    ctx.decrement();
                }
            });
        });

        Resource {
            state,
            set_state,
            trigger: set_trigger,
        }
    }

    pub fn refetch(&self) {
        self.trigger.update(|n| *n = n.wrapping_add(1));
    }

    /// Mutate the resource's data directly if available.
    /// Useful for optimistic UI updates.
    /// This will transition state to `Ready(new_data)`.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.set_state.update(|s| {
            let mut new_state = None;
            match s {
                ResourceState::Ready(data) => {
                    f(data);
                }
                ResourceState::Reloading(data) => {
                    f(data);
                    new_state = Some(ResourceState::Ready(data.clone()));
                }
                _ => {}
            }

            if let Some(ns) = new_state {
                *s = ns;
            }
        });
    }

    /// Set the resource's data directly.
    /// This transitions the state to `Ready(value)`.
    pub fn set(&self, value: T) {
        self.set_state.set(ResourceState::Ready(value));
    }

    /// Helper to check if the resource is currently `Loading`.
    pub fn loading(&self) -> bool {
        self.state.with(|s: &ResourceState<T, E>| s.is_loading())
    }

    /// Helper to get the last successful value, if any.
    pub fn value(&self) -> Option<T> {
        self.state
            .with(|s: &ResourceState<T, E>| s.as_option().cloned())
    }

    /// Helper to get data if available (Ready or Reloading)
    pub fn get_data(&self) -> Option<T> {
        self.state.with(|s| s.as_option().cloned())
    }

    pub fn map<U: Clone + PartialEq + 'static>(
        &self,
        f: impl Fn(Option<&T>) -> U + 'static,
    ) -> Memo<U> {
        let state = self.state;
        Memo::new(move |_| state.with(|s| f(s.as_option())))
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> RxBase for Resource<T, E> {
    type Value = Option<T>;

    #[inline(always)]
    fn id(&self) -> Option<crate::reactivity::NodeId> {
        self.state.id()
    }

    #[inline(always)]
    fn track(&self) {
        self.state.track();
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.state.is_disposed()
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.state.defined_at()
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.state.debug_name()
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> RxInternal for Resource<T, E> {
    type ReadOutput<'a>
        = RxGuard<'a, Option<T>, Option<T>>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.state
            .try_with_untracked(|s| RxGuard::Owned(s.as_option().cloned()))
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state.try_with_untracked(|s: &ResourceState<T, E>| {
            let val = s.as_option().cloned();
            fun(&val)
        })
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::AdaptiveWrapper;
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        false
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> WithUntracked for Resource<T, E> {
    #[inline(always)]
    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.rx_try_with_untracked(fun)
    }
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> IntoRx for Resource<T, E> {
    type Value = Option<T>;
    type RxType = Rx<Self, RxValue>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx(self, PhantomData)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        false
    }
    #[inline(always)]
    fn into_signal(self) -> crate::reactivity::Signal<Option<T>>
    where
        Self: 'static,
        T: Clone,
    {
        crate::reactivity::Signal::derive(move || self.get())
    }
}

// Note: GetUntracked and Get methods are now provided as default methods in the Read trait.

// --- Suspense ---

#[derive(Clone, Copy)]
pub struct SuspenseContext {
    pub count: ReadSignal<usize>,
    pub set_count: WriteSignal<usize>,
}

impl Default for SuspenseContext {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn provide<T>(f: impl FnOnce() -> T) -> T {
        let mut result = None;
        crate::reactivity::create_scope(|| {
            let ctx = Self::new();
            crate::reactivity::provide_context(ctx);
            result = Some(f());
        });
        result.unwrap()
    }
}

pub fn use_suspense_context() -> Option<SuspenseContext> {
    use_context::<SuspenseContext>()
}
