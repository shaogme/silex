use std::{rc::Rc, time::Duration};

use silex_core::{
    reactivity::{Mutation, Resource},
    traits::{RxCloneData, RxGet},
};

use crate::net::{
    NetError,
    builder::{HttpClientBuilder, IntoNetValue, ValueResolver},
    codec::ResponseCodec,
    state::{RequestSpec, RetryPolicy},
};

#[cfg(feature = "net")]
use gloo_timers::future::sleep;

#[cfg(feature = "persistence")]
use crate::net::{codec::CacheCodec, state::CachePolicy};

macro_rules! impl_net_methods {
    () => {
        async fn fetch_once(&self, spec: RequestSpec) -> Result<T, NetError> {
            let response = self.transport.send(spec.clone()).await;
            match response {
                Ok(resp) => {
                    let value = self.response_codec.decode(&resp.raw_body)?;
                    #[cfg(feature = "persistence")]
                    if let Some(cache) = &self.cache {
                        if !matches!(cache.policy, CachePolicy::None) {
                            let _ = self.cache_store(&spec, value.clone());
                        }
                    }
                    self.notify_response(&spec, &resp);
                    Ok(value)
                }
                Err(err) => {
                    self.notify_error(&spec, &err);
                    Err(err)
                }
            }
        }

        pub async fn send(&self) -> Result<T, NetError> {
            let mut spec = self.resolve_spec();
            self.apply_interceptors(&mut spec);

            #[cfg(feature = "persistence")]
            if let Some(cache) = &self.cache {
                if matches!(cache.policy, CachePolicy::CacheFirst) {
                    if let Some(value) = self.cached_value(&spec) {
                        return Ok(value);
                    }
                }
                if matches!(cache.policy, CachePolicy::StaleWhileRevalidate) {
                    if let Some(value) = self.cached_value(&spec) {
                        let client = self.clone();
                        let spec_for_refresh = spec.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let _ = client.fetch_once(spec_for_refresh).await;
                        });
                        return Ok(value);
                    }
                }
            }

            let retry = self
                .retry
                .unwrap_or(RetryPolicy::new(1, Duration::from_millis(0)));
            let attempts = retry.max_attempts.max(1);
            let started_at = js_sys::Date::now();

            let mut last_err = None;
            for attempt in 1..=attempts {
                match self.fetch_once(spec.clone()).await {
                    Ok(value) => return Ok(value),
                    Err(err) => {
                        last_err = Some(err.clone());
                        if attempt < attempts && err.is_retryable() {
                            let delay = retry.delay_for_attempt(attempt);
                            if let Some(max_elapsed) = retry.max_elapsed {
                                let elapsed = Duration::from_millis(
                                    (js_sys::Date::now() - started_at) as u64,
                                );
                                let next_elapsed = elapsed.saturating_add(delay);
                                if elapsed >= max_elapsed || next_elapsed > max_elapsed {
                                    break;
                                }
                            }
                            self.notify_retry(&spec, attempt, delay, &err);
                            if delay > Duration::from_millis(0) {
                                sleep(delay).await;
                            }
                            continue;
                        }
                        break;
                    }
                }
            }

            let err = last_err.expect("attempts are always at least 1");
            #[cfg(feature = "persistence")]
            if let Some(value) = self.cached_value(&spec) {
                return Ok(value);
            }
            Err(err)
        }

        pub fn bearer_auth(self, token: impl IntoNetValue) -> Self {
            let resolver = token.into_net_value();
            self.header(
                "Authorization",
                ValueResolver::Dynamic(Rc::new(move || format!("Bearer {}", resolver.resolve()))),
            )
        }

        pub fn into_resource(self) -> Resource<T, NetError> {
            self.as_resource(())
        }

        pub fn as_resource<S>(self, source: S) -> Resource<T, NetError>
        where
            S: RxGet + 'static,
            S::Value: PartialEq + RxCloneData,
        {
            Resource::new(source, move |_| {
                let client = self.clone();
                async move { client.send().await }
            })
        }

        pub fn as_mutation(self) -> Mutation<(), T, NetError> {
            Mutation::new(move |_| {
                let client = self.clone();
                async move { client.send().await }
            })
        }

        pub fn as_mutation_with<Input, F>(self, f: F) -> Mutation<Input, T, NetError>
        where
            F: Fn(Input) -> Self + 'static,
            Input: 'static,
        {
            Mutation::new(move |input: Input| {
                let client = f(input);
                async move { client.send().await }
            })
        }
    };
}

#[cfg(all(feature = "json", feature = "persistence"))]
impl<T, C> HttpClientBuilder<T, C>
where
    T: Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned + 'static,
    C: ResponseCodec<T> + CacheCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(feature = "json", not(feature = "persistence")))]
impl<T, C> HttpClientBuilder<T, C>
where
    T: Clone + PartialEq + serde::Serialize + serde::de::DeserializeOwned + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(not(feature = "json"), feature = "persistence"))]
impl<T, C> HttpClientBuilder<T, C>
where
    T: Clone + PartialEq + 'static,
    C: ResponseCodec<T> + CacheCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}

#[cfg(all(not(feature = "json"), not(feature = "persistence")))]
impl<T, C> HttpClientBuilder<T, C>
where
    T: Clone + 'static,
    C: ResponseCodec<T> + Clone + 'static,
{
    impl_net_methods!();
}
