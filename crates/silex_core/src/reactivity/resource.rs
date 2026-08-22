use crate::callback::report_completion_error;
use crate::reactivity::ReactiveSource;
use crate::{
    ErrorHandlerInput, OwnerAccess, OwnerChild, ReactiveError, Rx, SilexError, SilexErrorKind,
    SilexResult,
    reactivity::{EffectPhase, ReadSignal, Signal},
    traits::{RuntimeScoped, RxBase, RxCloneData, RxData, RxError, RxGet, RxRead, RxValue},
    unwind_safe,
};
use std::{cell::Cell, future::Future, rc::Rc};

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
    state: Signal<'owner, ResourceState<T, E>>,
    trigger: Signal<'owner, usize>,
}

impl<'owner, T, E> Copy for Resource<'owner, T, E> {}

impl<'owner, T, E> Clone for Resource<'owner, T, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'owner, T, E> PartialEq for Resource<'owner, T, E> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && self.trigger == other.trigger
    }
}

impl<'owner, T, E> Eq for Resource<'owner, T, E> {}

pub trait ResourceFetcher<'owner, S> {
    type Data;
    type Error;
    type Future: Future<Output = Result<Self::Data, Self::Error>> + 'owner;

    fn fetch(&self, source: S) -> Self::Future;
}

/// A reactive source whose runtime provenance can be checked before resource
/// initialization allocates any target-side nodes.
pub trait ResourceSource<'owner>:
    RxGet + ReactiveSource<'owner> + RuntimeScoped + Clone + 'owner
where
    Self::Value: Sized + Clone,
{
}

impl<'owner, S> ResourceSource<'owner> for S
where
    S: RxGet + ReactiveSource<'owner> + RuntimeScoped + Clone + 'owner,
    S::Value: Sized + Clone,
{
}

pub struct ResourceBuilder<'owner> {
    owner: OwnerAccess<'owner>,
}

pub struct ResourceSourceBuilder<'owner, S> {
    owner: OwnerAccess<'owner>,
    source: S,
}

pub struct ResourceFetchBuilder<'owner, S, Fetcher> {
    owner: OwnerAccess<'owner>,
    source: S,
    fetcher: Fetcher,
    suspense: Option<SuspenseContext<'owner>>,
}

type ResourceHandles<'owner, T, E> = (Signal<'owner, ResourceState<T, E>>, Signal<'owner, usize>);

impl<'owner> ResourceBuilder<'owner> {
    pub fn source<S>(self, source: S) -> ResourceSourceBuilder<'owner, S>
    where
        S: ResourceSource<'owner>,
        S::Value: Sized + Clone,
    {
        ResourceSourceBuilder {
            owner: self.owner,
            source,
        }
    }
}

impl<'owner, S> ResourceSourceBuilder<'owner, S>
where
    S: ResourceSource<'owner>,
    S::Value: Sized + Clone,
{
    pub fn fetch<Fetcher>(self, fetcher: Fetcher) -> ResourceFetchBuilder<'owner, S, Fetcher> {
        ResourceFetchBuilder {
            owner: self.owner,
            source: self.source,
            fetcher,
            suspense: None,
        }
    }
}

impl<'owner, S, Fetcher> ResourceFetchBuilder<'owner, S, Fetcher>
where
    S: ResourceSource<'owner>,
    S::Value: Sized + Clone,
{
    pub fn suspense(mut self, suspense: SuspenseContext<'owner>) -> Self {
        self.suspense = Some(suspense);
        self
    }

    pub fn build<T, E, H>(self, error_handler: H) -> SilexResult<Resource<'owner, T, E>>
    where
        T: RxCloneData + 'static,
        E: RxError + 'static,
        Fetcher: ResourceFetcher<'owner, S::Value, Data = T, Error = E> + 'owner,
        H: Clone + ErrorHandlerInput<'owner>,
    {
        let Self {
            owner,
            source,
            fetcher,
            suspense,
        } = self;
        owner.validate_runtime(&source)?;
        if let Some(suspense) = suspense.as_ref() {
            owner.validate_runtime(suspense)?;
        }
        let handler_owner = error_handler.clone();
        let error_handler = error_handler.handler_ref();
        let _handler_lease = error_handler
            .lease()
            .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))?;
        let child = owner.create_owned_child()?;
        let child_owner = child.access();
        let result = Resource::initialize(child_owner, source, fetcher, suspense, handler_owner);
        match result {
            Ok((state, trigger)) => {
                match owner.on_owner_cleanup(
                    child,
                    |child| {
                        child
                            .close()
                            .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
                    },
                    error_handler,
                ) {
                    Ok(()) => Ok(Resource { state, trigger }),
                    Err(error) => {
                        let (error, child) = error.into_parts();
                        close_child_for_rollback(child);
                        Err(error)
                    }
                }
            }
            Err(error) => {
                let close_result = child.close();
                if close_result.is_err() {
                    // Drop retries the close and sends persistent failures to
                    // the existing owner diagnostic sink.
                }
                Err(error)
            }
        }
    }
}

fn close_child_for_rollback<'owner>(child: OwnerChild<'owner>) {
    if child.close().is_err() {
        // Dropping the adapter retries the close and sends persistent failures
        // to the existing owner diagnostic sink.
    }
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
    fn initialize<S, Fetcher, H>(
        owner: OwnerAccess<'owner>,
        source: S,
        fetcher: Fetcher,
        suspense: Option<SuspenseContext<'owner>>,
        error_handler: H,
    ) -> crate::SilexResult<ResourceHandles<'owner, T, E>>
    where
        S: ResourceSource<'owner>,
        S::Value: Sized + Clone,
        Fetcher: ResourceFetcher<'owner, S::Value, Data = T, Error = E> + 'owner,
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
        let state = owner.signal(ResourceState::Idle)?;
        let trigger = owner.signal(0usize)?;
        let request_id = Rc::new(Cell::new(0usize));
        let request_id_for_callback = request_id.clone();
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
                    state.set(next_state)?;
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
        let suspense_for_effect = suspense;
        let error_handler_for_effect = error_handler;
        let _effect = owner.effect(
            EffectPhase::Normal,
            move || -> SilexResult<()> {
                let input = source_for_effect.get()?;
                trigger_for_effect.track()?;
                let next_state = state_for_effect.with_untracked(|state| {
                    state
                        .as_option()
                        .cloned()
                        .map(ResourceState::Reloading)
                        .unwrap_or(ResourceState::Loading)
                })?;
                state.set(next_state)?;
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
                            Err(error) => report_completion_error(error, |error| {
                                let _ = completion_error_handler.handle(error);
                            }),
                        }
                    },
                    error_handler,
                )?;
                Ok(())
            },
            error_handler_for_effect,
        )?;

        Ok((state, trigger))
    }

    pub fn state(&self) -> ReadSignal<'owner, ResourceState<T, E>> {
        self.state.read_signal()
    }

    pub fn refetch(&self) -> SilexResult<()> {
        self.trigger.update(|value| *value = value.wrapping_add(1))
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) -> SilexResult<()> {
        self.state.update(|state| match state {
            ResourceState::Ready(value) | ResourceState::Reloading(value) => f(value),
            _ => {}
        })
    }

    pub fn set(&self, value: T) -> SilexResult<()> {
        self.state.set(ResourceState::Ready(value))
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

impl<'owner> Resource<'owner, (), SilexError> {
    pub fn builder(owner: OwnerAccess<'owner>) -> ResourceBuilder<'owner> {
        ResourceBuilder { owner }
    }
}

impl<'owner, T: RxData + 'owner, E: RxError + 'owner> RxValue for Resource<'owner, T, E> {
    type Value = Option<T>;
}

impl<'owner, T: RxData + 'owner, E: RxError + 'owner> RxBase for Resource<'owner, T, E> {
    fn track(&self) -> SilexResult<()> {
        self.state.track()
    }
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
    pub count: Signal<'owner, usize>,
}

impl<'owner> PartialEq for SuspenseContext<'owner> {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count
    }
}

impl<'owner> Eq for SuspenseContext<'owner> {}

impl<'owner> SuspenseContext<'owner> {
    pub fn new(owner: OwnerAccess<'owner>) -> crate::SilexResult<Self> {
        Ok(Self {
            count: owner.signal(0usize)?,
        })
    }

    pub fn increment(&self) -> SilexResult<()> {
        self.count.update(|count| *count += 1)
    }

    pub fn decrement(&self) -> SilexResult<()> {
        self.count.update(|count| *count = count.saturating_sub(1))
    }
}

impl<'owner> RuntimeScoped for SuspenseContext<'owner> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.count.owner_access()
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
