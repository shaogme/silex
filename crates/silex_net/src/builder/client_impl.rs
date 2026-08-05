use std::{cell::Cell, marker::PhantomData, rc::Rc, time::Duration};

use gloo_timers::future::sleep;
use silex_core::{
    CompletionToken, Mutation, ReactiveSource, Resource, RxGet, RxRead, SuspenseContext,
};

use crate::{
    NetError,
    builder::HttpClientBuilder,
    codec::ResponseCodec,
    state::{CachePolicy, RequestSpec, RetryPolicy},
};

#[cfg(feature = "persist")]
use crate::codec::CacheCodec;

#[derive(Clone)]
struct PreparedClient<T, C> {
    response_codec: C,
    transport: Rc<dyn crate::Transport>,
    before_send: Vec<crate::builder::BeforeSendHook>,
    after_response: Vec<crate::builder::AfterResponseHook>,
    on_retry: Vec<crate::builder::OnRetryHook>,
    on_error: Vec<crate::builder::OnErrorHook>,
    retry: Option<RetryPolicy>,
    marker: PhantomData<fn() -> T>,
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
    cache_token: Option<CompletionToken<T>>,
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
        let response = client.transport.send(spec.clone()).await;
        match response {
            Ok(response) => {
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
                    let _ = token.submit(value.clone());
                }
                return Ok(value);
            }
            Err(error) => {
                for hook in &client.on_error {
                    hook(&spec, &error);
                }
                last_error = Some(error.clone());
                if attempt < attempts && error.is_retryable() {
                    let delay = retry.delay_for_attempt(attempt);
                    if let Some(max_elapsed) = retry.max_elapsed {
                        let elapsed = Duration::from_millis(
                            (js_sys::Date::now() - started_at).max(0.0) as u64,
                        );
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
                break;
            }
        }
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

        #[cfg(feature = "persist")]
        fn cache_completion_token(&self, spec: &RequestSpec) -> Option<CompletionToken<T>>
        where
            C: CacheCodec<T>,
            T: Clone + PartialEq + 'static,
        {
            let store = self.ensure_cache(spec)?;
            let expected_key = spec.cache_key();
            let key_state = self.cache_key_state()?;
            let token = self.scope.completion(move |value: T| {
                if key_state.borrow().as_deref() == Some(expected_key.as_str()) {
                    store.set(value);
                }
            });
            Some(token)
        }

        pub async fn send(&self) -> Result<T, NetError> {
            self.validate_runtime_inputs()?;
            let mut spec = self.resolve_spec();
            let client = self.prepared();
            client.apply_interceptors(&mut spec);

            #[cfg(feature = "persist")]
            let cache_snapshot = self.cached_value(&spec);

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
                let store = self.ensure_cache(&spec);
                let completion = self.scope.completion(move |result: Result<T, NetError>| {
                    if let (Some(store), Ok(value)) = (store, result) {
                        store.set(value);
                    }
                });
                self.scope.spawn_scoped(async move {
                    let result = execute_prepared(refresh_client, refresh_spec, None, None).await;
                    let _ = completion.submit(result);
                });
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
                    self.cache_completion_token(&spec)
                }
                #[cfg(not(feature = "persist"))]
                {
                    None
                }
            };
            execute_prepared(client, spec, fallback, cache_token).await
        }

        pub fn into_resource(
            self,
            suspense: Option<SuspenseContext<'scope>>,
        ) -> Resource<'scope, T, NetError> {
            let source = self.scope.constant(());
            self.as_resource(source, suspense)
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
            inputs.extend(&silex_core::runtime_inputs_of(source.clone()));
            scope
                .try_validate_inputs(&inputs)
                .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

            let request_builder = self.clone();
            let request_source = scope
                .try_derived_from(request_inputs, move || {
                    request_builder.resolve_spec_tracked()
                })
                .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

            let mut initial_spec = self.resolve_spec();
            let initial_client = self.prepared();
            initial_client.apply_interceptors(&mut initial_spec);

            #[cfg(feature = "persist")]
            let cache_policy = self.cache_policy();
            #[cfg(not(feature = "persist"))]
            let cache_policy = None;
            #[cfg(feature = "persist")]
            let initial_cache = self.cached_value(&initial_spec);
            #[cfg(not(feature = "persist"))]
            let initial_cache = None;
            #[cfg(feature = "persist")]
            let cache_key_state = self.cache_key_state();
            #[cfg(feature = "persist")]
            let initial_cache_key = initial_spec.cache_key();
            let cache_token = {
                #[cfg(feature = "persist")]
                {
                    self.cache_completion_token(&initial_spec)
                }
                #[cfg(not(feature = "persist"))]
                {
                    None
                }
            };

            let first_cache = Rc::new(Cell::new(true));
            let first_cache_for_fetcher = first_cache.clone();
            let fetch_client = initial_client.clone();
            let cache_for_fetcher = initial_cache.clone();
            let fallback = if matches!(cache_policy, Some(CachePolicy::NetworkFirst)) {
                initial_cache.clone()
            } else {
                None
            };
            let combined_source = scope
                .try_derived_from(inputs, move || (source.get(), request_source.get()))
                .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;
            let resource = Resource::new(
                &scope,
                combined_source,
                move |(_, spec): (S::Value, RequestSpec)| {
                    let mut spec = spec;
                    fetch_client.apply_interceptors(&mut spec);
                    #[cfg(feature = "persist")]
                    let request_key = spec.cache_key();
                    #[cfg(feature = "persist")]
                    if let Some(key_state) = &cache_key_state {
                        *key_state.borrow_mut() = Some(request_key.clone());
                    }
                    let use_cache = first_cache_for_fetcher.replace(false)
                        && matches!(
                            cache_policy,
                            Some(CachePolicy::CacheFirst | CachePolicy::StaleWhileRevalidate)
                        );
                    let cached = use_cache.then(|| cache_for_fetcher.clone()).flatten();
                    let client = fetch_client.clone();
                    let fallback = if matches!(cache_policy, Some(CachePolicy::NetworkFirst)) && {
                        #[cfg(feature = "persist")]
                        {
                            request_key == initial_cache_key
                        }
                        #[cfg(not(feature = "persist"))]
                        {
                            true
                        }
                    } {
                        fallback.clone()
                    } else {
                        None
                    };
                    let cache_token = cache_token.clone();
                    Box::pin(async move {
                        if let Some(value) = cached {
                            Ok(value)
                        } else {
                            execute_prepared(client, spec, fallback, cache_token).await
                        }
                    })
                },
                suspense,
            );

            #[cfg(feature = "persist")]
            if matches!(cache_policy, Some(CachePolicy::StaleWhileRevalidate))
                && let Some(store) = self.ensure_cache(&initial_spec)
                && initial_cache.is_some()
            {
                let refresh_client = initial_client;
                let refresh_spec = initial_spec;
                let resource_for_refresh = resource;
                let expected_key = refresh_spec.cache_key();
                let key_state = self
                    .cache_key_state()
                    .expect("cache key state exists with a cache store");
                let completion = scope.completion(move |result: Result<T, NetError>| {
                    if key_state.borrow().as_deref() == Some(expected_key.as_str())
                        && let Ok(value) = result
                    {
                        store.set(value.clone());
                        resource_for_refresh.set(value);
                    }
                });
                scope.spawn_scoped(async move {
                    let result = execute_prepared(refresh_client, refresh_spec, None, None).await;
                    let _ = completion.submit(result);
                });
            }

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
            Ok(Mutation::new(&self.scope, move |_| {
                let mut spec = builder.resolve_spec();
                let client = builder.prepared();
                client.apply_interceptors(&mut spec);
                #[cfg(feature = "persist")]
                let fallback = if matches!(builder.cache_policy(), Some(CachePolicy::NetworkFirst))
                {
                    builder.cached_value(&spec)
                } else {
                    None
                };
                #[cfg(not(feature = "persist"))]
                let fallback = None;
                let cache_token = {
                    #[cfg(feature = "persist")]
                    {
                        builder.cache_completion_token(&spec)
                    }
                    #[cfg(not(feature = "persist"))]
                    {
                        None
                    }
                };
                async move { execute_prepared(client, spec, fallback, cache_token).await }
            }))
        }

        pub fn as_mutation(&self) -> Mutation<'scope, (), T, NetError> {
            self.try_as_mutation()
                .unwrap_or_else(|error| panic!("创建 HTTP Mutation 失败: {error:?}"))
        }

        pub fn as_mutation_with<Input, F>(self, factory: F) -> Mutation<'scope, Input, T, NetError>
        where
            F: Fn(Input) -> Self + 'scope,
            Input: 'scope,
        {
            let scope = self.scope;
            Mutation::new(&scope, move |input: Input| {
                let builder = factory(input);
                let prepared = builder.validate_runtime_inputs_for(scope).map(|()| {
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
                            builder.cache_completion_token(&spec)
                        }
                        #[cfg(not(feature = "persist"))]
                        {
                            None
                        }
                    };
                    (client, spec, fallback, cache_token)
                });
                async move {
                    match prepared {
                        Ok((client, spec, fallback, cache_token)) => {
                            execute_prepared(client, spec, fallback, cache_token).await
                        }
                        Err(error) => Err(error),
                    }
                }
            })
        }
    };
}

#[cfg(all(feature = "json", feature = "persist"))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned + 'static,
    C: ResponseCodec<T> + CacheCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(feature = "json", not(feature = "persist")))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(not(feature = "json"), feature = "persist"))]
impl<'scope, T, C> HttpClientBuilder<'scope, T, C>
where
    T: Clone + PartialEq + 'static,
    C: ResponseCodec<T> + CacheCodec<T> + Clone + 'static,
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
