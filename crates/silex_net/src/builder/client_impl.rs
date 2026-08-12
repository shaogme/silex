use std::{
    cell::Cell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    time::Duration,
};

use gloo_timers::future::sleep;
use silex_core::{
    CallbackInvokeError, CompletionOnce, ErrorReporter, Mutation, ReactiveSource, Resource, RxGet,
    RxRead, SilexError, SuspenseContext, runtime_inputs_of, unwind_safe,
};

use crate::{
    NetError, Transport,
    builder::HttpClientBuilder,
    codec::ResponseCodec,
    state::{CachePolicy, RequestSpec, RetryPolicy},
};

#[derive(Clone)]
struct PreparedClient<T, C> {
    response_codec: C,
    transport: Rc<dyn Transport>,
    before_send: Vec<crate::builder::BeforeSendHook>,
    after_response: Vec<crate::builder::AfterResponseHook>,
    on_retry: Vec<crate::builder::OnRetryHook>,
    on_error: Vec<crate::builder::OnErrorHook>,
    retry: Option<RetryPolicy>,
    marker: PhantomData<fn() -> T>,
}

fn submit_once<T: 'static>(
    token: &CompletionOnce<T>,
    value: T,
    error_handler: ErrorReporter<'static>,
) {
    let result = token.submit(value);
    let Err(error) = result else {
        return;
    };
    let error = match error {
        CallbackInvokeError::Runtime(error) => SilexError::Reactivity(error),
        CallbackInvokeError::User(error) => error,
    };
    let handler_result = catch_unwind(AssertUnwindSafe(|| error_handler.handle(error)));
    if let Err(handler_panic) = handler_result {
        let _ = catch_unwind(AssertUnwindSafe(|| token.cancel()));
        resume_unwind(handler_panic);
    }
}

fn erase_error_handler<'scope>(handler: ErrorReporter<'scope>) -> ErrorReporter<'static> {
    // SAFETY: Completion destinations reject stale submissions before this
    // owner-bound handler is accessed after scope disposal.
    unsafe { std::mem::transmute(handler) }
}

impl<T, C> PreparedClient<T, C> {
    fn apply_interceptors(&self, spec: &mut RequestSpec) {
        for hook in &self.before_send {
            hook(spec);
        }
    }
}

async fn execute_prepared<T, C>(
    client: PreparedClient<T, C>,
    spec: RequestSpec,
    fallback: Option<T>,
    cache_token: Option<CompletionOnce<T>>,
    error_handler: ErrorReporter<'static>,
) -> Result<T, NetError>
where
    T: Clone + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    let retry = client
        .retry
        .unwrap_or(RetryPolicy::new(1, Duration::from_millis(0)));
    let attempts = retry.max_attempts.max(1);
    let started_at = js_sys::Date::now();
    let mut last_error = None;

    for attempt in 1..=attempts {
        let response = match client.transport.send(spec.clone()).await {
            Ok(response) if response.ok() => Ok(response),
            Ok(response) => Err(NetError::HttpStatus {
                status: response.status,
                body: response.raw_body,
            }),
            Err(error) => Err(error),
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                for hook in &client.on_error {
                    hook(&spec, &error);
                }
                last_error = Some(error.clone());
                if attempt >= attempts || !error.is_retryable() {
                    break;
                }
                let delay = retry.delay_for_attempt(attempt);
                if let Some(max_elapsed) = retry.max_elapsed {
                    let elapsed =
                        Duration::from_millis((js_sys::Date::now() - started_at).max(0.0) as u64);
                    let next_elapsed = elapsed.saturating_add(delay);
                    if elapsed >= max_elapsed || next_elapsed > max_elapsed {
                        break;
                    }
                }
                for hook in &client.on_retry {
                    hook(&spec, attempt, delay, &error);
                }
                if delay > Duration::from_millis(0) {
                    sleep(delay).await;
                }
                continue;
            }
        };

        let value = match client.response_codec.decode(&response.raw_body) {
            Ok(value) => value,
            Err(error) => {
                for hook in &client.on_error {
                    hook(&spec, &error);
                }
                return Err(error);
            }
        };
        for hook in &client.after_response {
            hook(&spec, &response);
        }
        if let Some(token) = &cache_token {
            submit_once(token, value.clone(), error_handler);
        }
        return Ok(value);
    }

    if let Some(value) = fallback {
        return Ok(value);
    }
    Err(last_error.expect("retry always performs at least one attempt"))
}

macro_rules! impl_net_methods {
    () => {
        fn prepared(&self) -> PreparedClient<T, C> {
            PreparedClient {
                response_codec: self.response_codec.clone(),
                transport: self.transport.clone(),
                before_send: self.before_send.clone(),
                after_response: self.after_response.clone(),
                on_retry: self.on_retry.clone(),
                on_error: self.on_error.clone(),
                retry: self.retry,
                marker: PhantomData,
            }
        }

        pub async fn send(&self) -> Result<T, NetError> {
            self.validate_runtime_inputs()?;
            let mut spec = self.resolve_spec();
            let client = self.prepared();
            client.apply_interceptors(&mut spec);

            #[cfg(feature = "persist")]
            let cache_binding = self.cache_binding(&spec);
            #[cfg(feature = "persist")]
            let cache_snapshot = cache_binding
                .as_ref()
                .and_then(|binding| binding.snapshot.clone());

            #[cfg(feature = "persist")]
            if matches!(self.cache_policy(), Some(CachePolicy::CacheFirst))
                && let Some(value) = cache_snapshot.clone()
            {
                return Ok(value);
            }

            #[cfg(feature = "persist")]
            if matches!(self.cache_policy(), Some(CachePolicy::StaleWhileRevalidate))
                && let Some(value) = cache_snapshot.clone()
            {
                let refresh_client = client.clone();
                let refresh_spec = spec.clone();
                let refresh_binding = self.cache_binding(&spec);
                let cache_token =
                    refresh_binding.map(|binding| self.cache_completion_once_for_binding(binding));
                let refresh_error_handler = erase_error_handler(self.error_handler);
                self.scope.spawn_scoped(
                    async move {
                        let _ = execute_prepared(
                            refresh_client,
                            refresh_spec,
                            None,
                            cache_token,
                            refresh_error_handler,
                        )
                        .await;
                    },
                    self.error_handler,
                );
                return Ok(value);
            }

            #[cfg(feature = "persist")]
            let fallback = if matches!(self.cache_policy(), Some(CachePolicy::NetworkFirst)) {
                cache_snapshot
            } else {
                None
            };
            #[cfg(not(feature = "persist"))]
            let fallback = None;

            let cache_token = {
                #[cfg(feature = "persist")]
                {
                    cache_binding.map(|binding| self.cache_completion_once_for_binding(binding))
                }
                #[cfg(not(feature = "persist"))]
                {
                    None
                }
            };
            execute_prepared(
                client,
                spec,
                fallback,
                cache_token,
                erase_error_handler(self.error_handler),
            )
            .await
        }

        pub fn into_resource(
            self,
            suspense: Option<SuspenseContext<'scope>>,
        ) -> Resource<'scope, T, NetError> {
            self.try_into_resource(suspense)
                .unwrap_or_else(|error| panic!("创建 HTTP Resource 失败: {error:?}"))
        }

        pub fn try_into_resource(
            self,
            suspense: Option<SuspenseContext<'scope>>,
        ) -> Result<Resource<'scope, T, NetError>, NetError> {
            self.validate_runtime_inputs()?;
            let source = self.scope.constant(());
            self.try_as_resource(source, suspense)
        }

        pub fn try_as_resource<S>(
            self,
            source: S,
            suspense: Option<SuspenseContext<'scope>>,
        ) -> Result<Resource<'scope, T, NetError>, NetError>
        where
            S: RxRead + ReactiveSource<'scope> + Clone + 'scope,
            S::Value: Clone + PartialEq + 'static,
        {
            let scope = self.scope;
            let request_inputs = self.runtime_inputs();
            let mut inputs = request_inputs.clone();
            inputs.extend(&runtime_inputs_of(source.clone()));
            scope
                .try_validate_inputs(&inputs)
                .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

            let error_handler = self.error_handler;
            let request_builder = self.clone();
            let request_source = scope
                .derived_from(
                    request_inputs,
                    move || Ok(request_builder.resolve_spec_tracked()),
                    error_handler,
                )
                .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

            #[cfg(feature = "persist")]
            let cache_policy = self.cache_policy();
            #[cfg(not(feature = "persist"))]
            let cache_policy = None;
            let fetch_client = self.prepared();
            let fetch_error_handler = error_handler;
            let completion_error_handler = erase_error_handler(fetch_error_handler);
            #[cfg(feature = "persist")]
            let fetch_builder = self.clone();
            let resource_generation = Rc::new(Cell::new(0usize));
            let resource_slot = Rc::new(Cell::new(None::<Resource<'scope, T, NetError>>));
            let resource_generation_for_fetcher = resource_generation.clone();
            let resource_slot_for_fetcher = resource_slot.clone();
            let combined_source = scope
                .derived_from(
                    inputs,
                    move || Ok((source.try_get()?, request_source.try_get()?)),
                    error_handler,
                )
                .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;
            let resource = Resource::new(
                scope,
                combined_source,
                move |(_, spec): (S::Value, RequestSpec)| {
                    let mut spec = spec;
                    fetch_client.apply_interceptors(&mut spec);
                    let generation = resource_generation_for_fetcher
                        .get()
                        .checked_add(1)
                        .expect("HTTP Resource generation exhausted");
                    resource_generation_for_fetcher.set(generation);

                    #[cfg(feature = "persist")]
                    let cache_binding = fetch_builder.cache_binding(&spec);
                    #[cfg(feature = "persist")]
                    let cache_snapshot = cache_binding
                        .as_ref()
                        .and_then(|binding| binding.snapshot.clone());
                    #[cfg(not(feature = "persist"))]
                    let cache_snapshot = None;

                    let cached = cache_snapshot.clone().filter(|_| {
                        matches!(
                            cache_policy,
                            Some(CachePolicy::CacheFirst | CachePolicy::StaleWhileRevalidate)
                        )
                    });
                    let fallback = cache_snapshot
                        .filter(|_| matches!(cache_policy, Some(CachePolicy::NetworkFirst)));

                    if matches!(cache_policy, Some(CachePolicy::StaleWhileRevalidate))
                        && cached.is_some()
                    {
                        #[cfg(feature = "persist")]
                        let refresh_binding = fetch_builder.cache_binding(&spec);
                        #[cfg(feature = "persist")]
                        let refresh_cache_token = refresh_binding.map(|binding| {
                            fetch_builder.cache_completion_once_for_binding(binding)
                        });
                        #[cfg(not(feature = "persist"))]
                        let refresh_cache_token = None;
                        let refresh_client = fetch_client.clone();
                        let refresh_spec = spec.clone();
                        let resource_generation_for_completion =
                            resource_generation_for_fetcher.clone();
                        let resource_slot_for_completion = resource_slot_for_fetcher.clone();
                        let completion = scope.completion_once(unwind_safe(
                            move |result: Result<T, NetError>| {
                                if resource_generation_for_completion.get() == generation
                                    && let Ok(value) = result
                                    && let Some(resource) = resource_slot_for_completion.get()
                                {
                                    resource.set(value);
                                }
                                Ok(())
                            },
                        ));
                        let refresh_error_handler = completion_error_handler;
                        scope.spawn_scoped(
                            async move {
                                sleep(Duration::from_millis(0)).await;
                                let result = execute_prepared(
                                    refresh_client,
                                    refresh_spec,
                                    None,
                                    refresh_cache_token,
                                    refresh_error_handler,
                                )
                                .await;
                                submit_once(&completion, result, refresh_error_handler);
                            },
                            fetch_error_handler,
                        );
                    }

                    let cache_token = if cached.is_some() {
                        None
                    } else {
                        #[cfg(feature = "persist")]
                        {
                            cache_binding.map(|binding| {
                                fetch_builder.cache_completion_once_for_binding(binding)
                            })
                        }
                        #[cfg(not(feature = "persist"))]
                        {
                            None
                        }
                    };
                    let client = fetch_client.clone();
                    Box::pin(async move {
                        if let Some(value) = cached {
                            Ok(value)
                        } else {
                            execute_prepared(
                                client,
                                spec,
                                fallback,
                                cache_token,
                                completion_error_handler,
                            )
                            .await
                        }
                    })
                },
                suspense,
                error_handler,
            );

            resource_slot.set(Some(resource));

            Ok(resource)
        }

        pub fn as_resource<S>(
            self,
            source: S,
            suspense: Option<SuspenseContext<'scope>>,
        ) -> Resource<'scope, T, NetError>
        where
            S: RxRead + ReactiveSource<'scope> + Clone + 'scope,
            S::Value: Clone + PartialEq + 'static,
        {
            self.try_as_resource(source, suspense)
                .unwrap_or_else(|error| panic!("创建 HTTP Resource 失败: {error:?}"))
        }

        pub fn try_as_mutation(&self) -> Result<Mutation<'scope, (), T, NetError>, NetError> {
            self.validate_runtime_inputs()?;
            let builder = self.clone();
            let completion_error_handler = erase_error_handler(self.error_handler);
            Ok(Mutation::new(
                self.scope,
                move |_| {
                    let mut spec = builder.resolve_spec();
                    let client = builder.prepared();
                    client.apply_interceptors(&mut spec);
                    #[cfg(feature = "persist")]
                    let fallback =
                        if matches!(builder.cache_policy(), Some(CachePolicy::NetworkFirst)) {
                            builder.cached_value(&spec)
                        } else {
                            None
                        };
                    #[cfg(not(feature = "persist"))]
                    let fallback = None;
                    let cache_token = {
                        #[cfg(feature = "persist")]
                        {
                            builder
                                .cache_binding(&spec)
                                .map(|binding| builder.cache_completion_once_for_binding(binding))
                        }
                        #[cfg(not(feature = "persist"))]
                        {
                            None
                        }
                    };
                    async move {
                        execute_prepared(
                            client,
                            spec,
                            fallback,
                            cache_token,
                            completion_error_handler,
                        )
                        .await
                    }
                },
                self.error_handler,
            ))
        }

        pub fn as_mutation(&self) -> Mutation<'scope, (), T, NetError> {
            self.try_as_mutation()
                .unwrap_or_else(|error| panic!("创建 HTTP Mutation 失败: {error:?}"))
        }

        pub fn as_mutation_with<Input, F>(self, factory: F) -> Mutation<'scope, Input, T, NetError>
        where
            F: Fn(Input) -> Result<Self, NetError> + 'scope,
            Input: 'scope,
        {
            let scope = self.scope;
            let completion_error_handler = erase_error_handler(self.error_handler);
            Mutation::new_with_prepare(
                scope,
                move |input: Input| {
                    let builder = factory(input)?;
                    builder.validate_runtime_inputs_for(scope)?;
                    let mut spec = builder.resolve_spec();
                    let client = builder.prepared();
                    client.apply_interceptors(&mut spec);
                    #[cfg(feature = "persist")]
                    let fallback =
                        if matches!(builder.cache_policy(), Some(CachePolicy::NetworkFirst)) {
                            builder.cached_value(&spec)
                        } else {
                            None
                        };
                    #[cfg(not(feature = "persist"))]
                    let fallback = None;
                    let cache_token = {
                        #[cfg(feature = "persist")]
                        {
                            builder
                                .cache_binding(&spec)
                                .map(|binding| builder.cache_completion_once_for_binding(binding))
                        }
                        #[cfg(not(feature = "persist"))]
                        {
                            None
                        }
                    };
                    Ok(async move {
                        execute_prepared(
                            client,
                            spec,
                            fallback,
                            cache_token,
                            completion_error_handler,
                        )
                        .await
                    })
                },
                self.error_handler,
            )
        }
    };
}

#[cfg(all(feature = "json", feature = "persist"))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(feature = "json", not(feature = "persist")))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(not(feature = "json"), feature = "persist"))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(not(feature = "json"), not(feature = "persist")))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}
