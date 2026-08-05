use crate::reactivity::ReactiveSource;
use crate::{
    Scope, SilexError,
    reactivity::{Memo, ReadSignal, RwSignal, WriteSignal},
    traits::{RxBase, RxCloneData, RxData, RxError, RxGet, RxRead, RxValue},
};
use silex_reactivity::RuntimeInputs;
use std::{cell::Cell, future::Future, marker::PhantomData, rc::Rc};

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

fn resolve_resource_result<T, E>(
    current_id: usize,
    id: usize,
    result: Result<T, E>,
) -> Option<ResourceState<T, E>> {
    (current_id == id).then(|| match result {
        Ok(value) => ResourceState::Ready(value),
        Err(error) => ResourceState::Error(error),
    })
}

struct ResourceCompletion<T, E> {
    id: usize,
    result: Result<T, E>,
    settled: Rc<Cell<bool>>,
}

pub struct Resource<'scope, T, E = SilexError> {
    pub state: ReadSignal<'scope, ResourceState<T, E>>,
    set_state: WriteSignal<'scope, ResourceState<T, E>>,
    trigger: RwSignal<'scope, usize>,
    marker: PhantomData<fn() -> &'scope ()>,
}

impl<'scope, T, E> Copy for Resource<'scope, T, E> {}

impl<'scope, T, E> Clone for Resource<'scope, T, E> {
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

impl<'scope, T, E> Resource<'scope, T, E>
where
    T: RxCloneData + 'static,
    E: RxError + 'static,
{
    pub fn new<S, R, Fetcher>(
        scope: Scope<'scope>,
        source: R,
        fetcher: Fetcher,
        suspense: Option<SuspenseContext<'scope>>,
    ) -> Self
    where
        S: Clone + PartialEq + 'static,
        R: RxRead<Value = S> + ReactiveSource<'scope> + Clone + 'scope,
        Fetcher: ResourceFetcher<S, Data = T, Error = E> + 'scope,
    {
        let mut inputs = source.clone().into_promotion_plan().inputs();
        if let Some(context) = suspense.as_ref() {
            inputs.push(context.count.inner.runtime_input());
        }
        scope.assert_inputs(&inputs);
        let (state, set_state) = scope.signal(ResourceState::Idle);
        let trigger = scope.rw_signal(0usize);
        inputs.push(trigger.read_signal().inner.runtime_input());
        let request_id = Rc::new(Cell::new(0usize));
        let request_id_for_callback = request_id.clone();
        let set_state_for_callback = set_state;
        let suspense_for_callback = suspense;
        let completion = scope.completion_sender(move |message: ResourceCompletion<T, E>| {
            if message.settled.replace(true) {
                return;
            }
            if let Some(next_state) =
                resolve_resource_result(request_id_for_callback.get(), message.id, message.result)
            {
                set_state_for_callback.set(next_state);
            }
            if let Some(context) = suspense_for_callback {
                context.decrement();
            }
        });

        let source_for_effect = source.clone();
        let trigger_for_effect = trigger.read_signal();
        let request_id_for_effect = request_id.clone();
        let state_for_effect = state;
        let set_state_for_effect = set_state;
        let suspense_for_effect = suspense;
        let _effect = scope.effect_from(inputs, move || {
            let input = source_for_effect.get();
            let _ = trigger_for_effect.get();
            let next_state = state_for_effect.with_untracked(|state| {
                state
                    .as_option()
                    .cloned()
                    .map(ResourceState::Reloading)
                    .unwrap_or(ResourceState::Loading)
            });
            set_state_for_effect.set(next_state);
            if let Some(context) = suspense_for_effect {
                context.increment();
            }
            let settled = Rc::new(Cell::new(false));
            let settled_for_cleanup = settled.clone();
            let suspense_for_cleanup = suspense_for_effect;
            scope.on_cleanup(move || {
                if !settled_for_cleanup.replace(true)
                    && let Some(context) = suspense_for_cleanup
                {
                    context.decrement();
                }
            });
            let id = request_id_for_effect
                .get()
                .checked_add(1)
                .expect("Resource request id exhausted");
            request_id_for_effect.set(id);
            let future = fetcher.fetch(input);
            let completion = completion.clone();
            scope.spawn_scoped(async move {
                let _ = completion.submit(ResourceCompletion {
                    id,
                    result: future.await,
                    settled,
                });
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

    pub fn map<U, F>(&self, scope: Scope<'scope>, f: F) -> Memo<'scope, U>
    where
        U: PartialEq + 'static,
        F: Fn(Option<&T>) -> U + 'scope,
    {
        let resource = *self;
        let inputs = RuntimeInputs::single(resource.state.inner.runtime_input());
        scope.memo_from(inputs, move |_| {
            resource.state.with(|state| f(state.as_option()))
        })
    }
}

impl<'scope, T: RxData + 'scope, E: RxError + 'scope> RxValue for Resource<'scope, T, E> {
    type Value = Option<T>;
}

impl<'scope, T: RxData + 'scope, E: RxError + 'scope> RxBase for Resource<'scope, T, E> {
    fn track(&self) {
        self.state.track();
    }

    fn is_alive(&self) -> bool {
        self.state.is_alive()
    }
}

impl<'scope, T: RxCloneData + 'scope, E: RxError + 'scope> RxRead for Resource<'scope, T, E> {
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state.try_with(|state| f(&state.as_option().cloned()))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.state
            .try_with_untracked(|state| f(&state.as_option().cloned()))
    }
}

#[derive(Clone, Copy)]
pub struct SuspenseContext<'scope> {
    pub count: ReadSignal<'scope, usize>,
    set_count: WriteSignal<'scope, usize>,
}

impl<'scope> SuspenseContext<'scope> {
    pub fn new(scope: Scope<'scope>) -> Self {
        let (count, set_count) = scope.signal(0usize);
        Self { count, set_count }
    }

    pub fn increment(&self) {
        self.set_count.update(|count| *count += 1);
    }

    pub fn decrement(&self) {
        let _ = self
            .set_count
            .try_update(|count| *count = count.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourceState, resolve_resource_result};

    #[test]
    fn stale_resource_result_does_not_replace_current_request() {
        assert_eq!(resolve_resource_result(2, 1, Ok::<_, ()>("stale")), None);
        assert_eq!(
            resolve_resource_result(2, 2, Ok::<_, ()>("current")),
            Some(ResourceState::Ready("current"))
        );
    }
}
