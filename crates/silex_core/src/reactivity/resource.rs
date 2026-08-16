use crate::reactivity::ReactiveSource;
use crate::{
    ErrorHandlerInput, OwnerAccess, Rx, SilexError, SilexErrorKind, SilexResult,
    reactivity::{ReadSignal, RwSignal, WriteSignal},
    traits::{RxCloneData, RxData, RxError, RxGet, RxRead, RxValue},
    unwind_safe,
};
use silex_reactivity::{CallbackInvokeError, ReactiveError};
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

    pub fn into_value(self) -> SilexResult<T> {
        match self {
            Self::Ready(value) | Self::Reloading(value) => Ok(value),
            _ => Err(SilexError::fatal(SilexErrorKind::Framework(
                "resource state does not contain data".into(),
            ))),
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

pub struct Resource<'owner, T, E = SilexError> {
    pub state: ReadSignal<'owner, ResourceState<T, E>>,
    set_state: WriteSignal<'owner, ResourceState<T, E>>,
    trigger: RwSignal<'owner, usize>,
    marker: PhantomData<fn() -> &'owner ()>,
}

impl<'owner, T, E> Copy for Resource<'owner, T, E> {}

impl<'owner, T, E> Clone for Resource<'owner, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'owner, T, E> PartialEq for Resource<'owner, T, E> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
            && self.set_state == other.set_state
            && self.trigger == other.trigger
    }
}

impl<'owner, T, E> Eq for Resource<'owner, T, E> {}

pub trait ResourceFetcher<'owner, S> {
    type Data;
    type Error;
    type Future: Future<Output = Result<Self::Data, Self::Error>> + 'owner;

    fn fetch(&self, source: S) -> Self::Future;
}

impl<'owner, S, T, E, F, Fut> ResourceFetcher<'owner, S> for F
where
    F: Fn(S) -> Fut + 'owner,
    Fut: Future<Output = Result<T, E>> + 'owner,
{
    type Data = T;
    type Error = E;
    type Future = Fut;

    fn fetch(&self, source: S) -> Self::Future {
        self(source)
    }
}

impl<'owner, T, E> Resource<'owner, T, E>
where
    T: RxCloneData + 'static,
    E: RxError + 'static,
{
    pub fn new<S, R, Fetcher, H>(
        owner: OwnerAccess<'owner>,
        source: R,
        fetcher: Fetcher,
        suspense: Option<SuspenseContext<'owner>>,
        error_handler: H,
    ) -> crate::SilexResult<Self>
    where
        S: Clone + PartialEq + 'static,
        R: RxRead<Value = S> + ReactiveSource<'owner> + Clone + 'owner,
        Fetcher: ResourceFetcher<'owner, S, Data = T, Error = E> + 'owner,
        H: Clone + ErrorHandlerInput<'owner>,
    {
        let handler_owner = error_handler.clone();
        let error_handler = error_handler.handler_ref();
        owner.on_cleanup(
            move || {
                drop(handler_owner);
                Ok::<(), SilexError>(())
            },
            error_handler,
        )?;
        let (state, set_state) = owner.signal(ResourceState::Idle)?;
        let trigger = owner.rw_signal(0usize)?;
        let request_id = Rc::new(Cell::new(0usize));
        let request_id_for_callback = request_id.clone();
        let set_state_for_callback = set_state;
        let suspense_for_callback = suspense;
        let completion =
            owner.completion_sender(unwind_safe(move |message: ResourceCompletion<T, E>| {
                if message.settled.replace(true) {
                    return Ok(());
                }
                if let Some(next_state) = resolve_resource_result(
                    request_id_for_callback.get(),
                    message.id,
                    message.result,
                ) {
                    set_state_for_callback.set(next_state)?;
                }
                if let Some(ctx) = suspense_for_callback {
                    ctx.decrement()?;
                }
                Ok(())
            }))?;

        let source_for_effect = source.clone();
        let trigger_for_effect = trigger.read_signal();
        let request_id_for_effect = request_id.clone();
        let state_for_effect = state;
        let set_state_for_effect = set_state;
        let suspense_for_effect = suspense;
        let error_handler_for_effect = error_handler;
        let _effect = owner.effect(
            move || -> SilexResult<()> {
                let input = source_for_effect.get()?;
                let _ = trigger_for_effect.get()?;
                let next_state = state_for_effect.with_untracked(|state| {
                    state
                        .as_option()
                        .cloned()
                        .map(ResourceState::Reloading)
                        .unwrap_or(ResourceState::Loading)
                })?;
                set_state_for_effect.set(next_state)?;
                if let Some(ctx) = suspense_for_effect {
                    ctx.increment()?;
                }
                let settled = Rc::new(Cell::new(false));
                let settled_for_cleanup = settled.clone();
                let suspense_for_cleanup = suspense_for_effect;
                owner.on_cleanup(
                    move || {
                        if !settled_for_cleanup.replace(true)
                            && let Some(ctx) = suspense_for_cleanup
                        {
                            ctx.decrement()?;
                        }
                        Ok(())
                    },
                    error_handler,
                )?;
                let id = request_id_for_effect.get().checked_add(1).ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Framework(
                        "Resource request id exhausted".into(),
                    ))
                })?;
                request_id_for_effect.set(id);
                let future = fetcher.fetch(input);
                let completion = completion.clone();
                let completion_error_handler = error_handler;
                owner.spawn_scoped(
                    async move {
                        let result = completion.submit(ResourceCompletion {
                            id,
                            result: future.await,
                            settled,
                        });
                        match result {
                            Ok(_) => {}
                            Err(CallbackInvokeError::Runtime(error)) => {
                                let _ = completion_error_handler.handle(SilexError::fatal(error));
                            }
                            Err(CallbackInvokeError::User(error)) => {
                                let _ = completion_error_handler.handle(error);
                            }
                            Err(CallbackInvokeError::Handler(error)) => {
                                let _ = completion_error_handler
                                    .handle(SilexError::fatal(ReactiveError::Handler(error)));
                            }
                            Err(CallbackInvokeError::Close(error)) => {
                                let _ = completion_error_handler
                                    .handle(SilexError::fatal(SilexErrorKind::Close(error)));
                            }
                        }
                    },
                    error_handler,
                )?;
                Ok(())
            },
            error_handler_for_effect,
        )?;

        Ok(Self {
            state,
            set_state,
            trigger,
            marker: PhantomData,
        })
    }

    pub fn refetch(&self) -> SilexResult<()> {
        self.trigger.update(|value| *value = value.wrapping_add(1))
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) -> SilexResult<()> {
        self.set_state.update(|state| match state {
            ResourceState::Ready(value) | ResourceState::Reloading(value) => f(value),
            _ => {}
        })
    }

    pub fn set(&self, value: T) -> SilexResult<()> {
        self.set_state.set(ResourceState::Ready(value))
    }

    pub fn loading(&self) -> crate::SilexResult<bool> {
        self.state.with(ResourceState::is_loading)
    }

    pub fn value(&self) -> crate::SilexResult<Option<T>> {
        self.state.with(|state| state.as_option().cloned())
    }

    pub fn get_data(&self) -> crate::SilexResult<Option<T>> {
        self.value()
    }

    pub fn map<U, F, H>(
        &self,
        owner: OwnerAccess<'owner>,
        f: F,
        error_handler: H,
    ) -> crate::SilexResult<Rx<'owner, U>>
    where
        U: 'owner,
        F: Fn(Option<&T>) -> U + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let resource = *self;
        owner
            .computed_always(
                move || resource.state.with(|state| f(state.as_option())),
                error_handler,
            )
            .map(crate::Computed::into_rx)
    }
}

impl<'owner, T: RxData + 'owner, E: RxError + 'owner> RxValue for Resource<'owner, T, E> {
    type Value = Option<T>;
}

impl<'owner, T: RxCloneData + 'owner, E: RxError + 'owner> RxRead for Resource<'owner, T, E> {
    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> crate::SilexResult<U> {
        self.state.with(|state| f(&state.as_option().cloned()))
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> crate::SilexResult<U> {
        self.state
            .with_untracked(|state| f(&state.as_option().cloned()))
    }
}

#[derive(Clone, Copy)]
pub struct SuspenseContext<'owner> {
    pub count: ReadSignal<'owner, usize>,
    set_count: WriteSignal<'owner, usize>,
}

impl<'owner> PartialEq for SuspenseContext<'owner> {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count && self.set_count == other.set_count
    }
}

impl<'owner> Eq for SuspenseContext<'owner> {}

impl<'owner> SuspenseContext<'owner> {
    pub fn new(owner: OwnerAccess<'owner>) -> crate::SilexResult<Self> {
        let (count, set_count) = owner.signal(0usize)?;
        Ok(Self { count, set_count })
    }

    pub fn increment(&self) -> SilexResult<()> {
        self.set_count.update(|count| *count += 1)
    }

    pub fn decrement(&self) -> SilexResult<()> {
        self.set_count
            .update(|count| *count = count.saturating_sub(1))
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
