use std::cell::Cell;
use std::future::Future;
use std::panic::Location;
use std::rc::Rc;

use silex_reactivity::{on_cleanup, use_context};

use crate::SilexError;
use crate::traits::*;

use super::effect::Effect;
use super::signal::{ReadSignal, WriteSignal, signal};

// --- Resource ---

pub struct Resource<T: 'static, E: 'static = SilexError> {
    pub data: ReadSignal<Option<T>>,
    set_data: WriteSignal<Option<T>>,
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
            set_data,
            error,
            loading,
            trigger: set_trigger,
        }
    }

    pub fn refetch(&self) {
        self.trigger.update(|n| *n = n.wrapping_add(1));
    }

    /// Mutate the resource's data directly.
    /// Useful for optimistic UI updates.
    pub fn update(&self, f: impl FnOnce(&mut Option<T>)) {
        self.set_data.update(f);
    }

    /// Set the resource's data directly.
    pub fn set(&self, value: Option<T>) {
        self.set_data.set(value);
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
