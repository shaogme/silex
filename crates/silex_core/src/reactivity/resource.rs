use crate::{
    Rx, Scope, SilexError,
    reactivity::{Memo, ReadSignal, RwSignal, WriteSignal},
    traits::{IntoRx, IntoSignal, RxBase, RxCloneData, RxData, RxError, RxGet, RxRead, RxValue},
};
use std::{cell::Cell, future::Future, marker::PhantomData, rc::Rc};
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceState<T, E> {
    Idle,
    Loading,
    Ready(T),
    Reloading(T),
    Error(E),
}

impl<T, E> ResourceState<T, E> {
    pub fn as_option(&self) -> Option<&T> {
        match self {
            Self::Ready(value) | Self::Reloading(value) => Some(value),
            _ => None,
        }
    }

    pub fn unwrap(self) -> T {
        match self {
            Self::Ready(value) | Self::Reloading(value) => value,
            _ => panic!("ResourceState::unwrap called without data"),
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading | Self::Reloading(_))
    }
}

pub struct Resource<'scope, 'run, T, E = SilexError> {
    pub state: ReadSignal<'scope, 'run, ResourceState<T, E>>,
    set_state: WriteSignal<'scope, 'run, ResourceState<T, E>>,
    trigger: RwSignal<'scope, 'run, usize>,
    marker: PhantomData<fn() -> (&'scope (), &'run ())>,
}

impl<'scope, 'run, T, E> Copy for Resource<'scope, 'run, T, E> {}

impl<'scope, 'run, T, E> Clone for Resource<'scope, 'run, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

pub trait ResourceFetcher<S> {
    type Data;
    type Error;
    type Future: Future<Output = Result<Self::Data, Self::Error>> + 'static;

    fn fetch(&self, source: S) -> Self::Future;
}

impl<S, T, E, F, Fut> ResourceFetcher<S> for F
where
    F: Fn(S) -> Fut,
    Fut: Future<Output = Result<T, E>> + 'static,
{
    type Data = T;
    type Error = E;
    type Future = Fut;

    fn fetch(&self, source: S) -> Self::Future {
        self(source)
    }
}

impl<'scope, 'run, T, E> Resource<'scope, 'run, T, E>
where
    T: RxCloneData + 'static,
    E: RxError + 'static,
{
    pub fn new<S, R, Fetcher>(
        scope: &Scope<'scope, 'run>,
        source: R,
        fetcher: Fetcher,
        suspense: Option<SuspenseContext<'scope, 'run>>,
    ) -> Self
    where
        S: Clone + PartialEq + 'static,
        R: RxRead<Value = S> + Clone + 'scope,
        Fetcher: ResourceFetcher<S, Data = T, Error = E> + 'scope,
    {
        let (state, set_state) = scope.signal(ResourceState::Idle);
        let trigger = scope.rw_signal(0usize);
        let request_id = Rc::new(Cell::new(0usize));
        let request_id_for_callback = request_id.clone();
        let set_state_for_callback = set_state;
        let suspense_for_callback = suspense;
        let completion = scope.completion(move |(id, result): (usize, Result<T, E>)| {
            if request_id_for_callback.get() == id {
                set_state_for_callback.set(match result {
                    Ok(value) => ResourceState::Ready(value),
                    Err(error) => ResourceState::Error(error),
                });
            }
            if let Some(context) = suspense_for_callback {
                context.decrement();
            }
        });

        let source_for_effect = source.clone();
        let trigger_for_effect = trigger.read_signal();
        let request_id_for_effect = request_id.clone();
        let suspense_for_effect = suspense;
        let _effect = scope.effect(move |_: Option<()>| {
            let input = source_for_effect.get();
            let _ = trigger_for_effect.get();
            if let Some(context) = suspense_for_effect {
                context.increment();
            }
            let id = request_id_for_effect.get().wrapping_add(1);
            request_id_for_effect.set(id);
            let future = fetcher.fetch(input);
            let completion = completion.clone();
            spawn_local(async move {
                let _ = completion.submit((id, future.await));
            });
        });

        Self {
            state,
            set_state,
            trigger,
            marker: PhantomData,
        }
    }

    pub fn refetch(&self) {
        self.trigger.update(|value| *value = value.wrapping_add(1));
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.set_state.update(|state| match state {
            ResourceState::Ready(value) | ResourceState::Reloading(value) => f(value),
            _ => {}
        });
    }

    pub fn set(&self, value: T) {
        self.set_state.set(ResourceState::Ready(value));
    }

    pub fn loading(&self) -> bool {
        self.state.with(ResourceState::is_loading)
    }

    pub fn value(&self) -> Option<T> {
        self.state.with(|state| state.as_option().cloned())
    }

    pub fn get_data(&self) -> Option<T> {
        self.value()
    }

    pub fn map<U, F>(&self, scope: &Scope<'scope, 'run>, f: F) -> Memo<'scope, 'run, U>
    where
        U: PartialEq + 'static,
        F: Fn(Option<&T>) -> U + 'scope,
    {
        let resource = *self;
        scope.memo(move |_| resource.state.with(|state| f(state.as_option())))
    }
}

impl<'scope, 'run, T: RxData + 'run, E: RxError + 'run> RxValue for Resource<'scope, 'run, T, E> {
    type Value = Option<T>;
}

impl<'scope, 'run, T: RxData + 'run, E: RxError + 'run> RxBase for Resource<'scope, 'run, T, E> {
    fn track(&self) {
        self.state.track();
    }

    fn is_alive(&self) -> bool {
        self.state.is_alive()
    }
}

impl<'scope, 'run, T: RxCloneData + 'run, E: RxError + 'run> RxRead
    for Resource<'scope, 'run, T, E>
{
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state.try_with(|state| f(&state.as_option().cloned()))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state
            .try_with_untracked(|state| f(&state.as_option().cloned()))
    }
}

impl<'scope, 'run, T: RxCloneData + 'run + 'static, E: RxError + 'run + 'static>
    IntoRx<'scope, 'run> for Resource<'scope, 'run, T, E>
{
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, Option<T>> {
        let resource = self;
        let scope = *scope;
        scope.derived(move || resource.value())
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<'scope, 'run, T: RxCloneData + 'run + 'static, E: RxError + 'run + 'static>
    IntoSignal<'scope, 'run> for Resource<'scope, 'run, T, E>
{
    fn into_signal(
        self,
        scope: &Scope<'scope, 'run>,
    ) -> crate::reactivity::Signal<'scope, 'run, Option<T>> {
        self.into_rx(scope).into_signal(scope)
    }
}

#[derive(Clone, Copy)]
pub struct SuspenseContext<'scope, 'run> {
    pub count: ReadSignal<'scope, 'run, usize>,
    set_count: WriteSignal<'scope, 'run, usize>,
}

impl<'scope, 'run> SuspenseContext<'scope, 'run> {
    pub fn new(scope: &Scope<'scope, 'run>) -> Self {
        let (count, set_count) = scope.signal(0usize);
        Self { count, set_count }
    }

    pub fn increment(&self) {
        self.set_count.update(|count| *count += 1);
    }

    pub fn decrement(&self) {
        self.set_count
            .update(|count| *count = count.saturating_sub(1));
    }
}
