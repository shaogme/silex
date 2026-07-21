use crate::net::NetError;
use crate::net::backend::{HttpBackend, Transport};
#[cfg(feature = "persistence")]
use crate::net::codec::CacheCodec;
#[cfg(feature = "json")]
use crate::net::codec::NetJsonCodec;
use crate::net::codec::{ResponseCodec, TextCodec};
use crate::net::state::{CachePolicy, HttpMethod, HttpResponse, RequestBody, RequestSpec};
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;
pub type BeforeSendHook = Rc<dyn Fn(&mut RequestSpec)>;
pub type AfterResponseHook = Rc<dyn Fn(&RequestSpec, &HttpResponse)>;
pub type OnRetryHook = Rc<dyn Fn(&RequestSpec, u32, Duration, &NetError)>;
pub type OnErrorHook = Rc<dyn Fn(&RequestSpec, &NetError)>;

#[cfg(feature = "net")]
use gloo_timers::future::sleep;
use silex_core::reactivity::{Memo, ReadSignal, RwSignal, Signal};
use silex_core::traits::{RxCloneData, RxGet};
#[cfg(feature = "persistence")]
use std::cell::Cell;

#[cfg(feature = "persistence")]
use crate::persist::Persistent;

#[derive(Clone)]
pub enum ValueResolver {
    Static(String),
    Dynamic(Rc<dyn Fn() -> String>),
}

impl ValueResolver {
    fn resolve(&self) -> String {
        match self {
            Self::Static(value) => value.clone(),
            Self::Dynamic(fun) => fun(),
        }
    }
}

pub trait IntoNetValue {
    fn into_net_value(self) -> ValueResolver;
}

impl IntoNetValue for String {
    fn into_net_value(self) -> ValueResolver {
        ValueResolver::Static(self)
    }
}

impl IntoNetValue for &str {
    fn into_net_value(self) -> ValueResolver {
        ValueResolver::Static(self.to_string())
    }
}

macro_rules! impl_into_net_value_for_rx {
    ($ty:ty) => {
        impl<T> IntoNetValue for $ty
        where
            T: ToString + Clone + 'static,
        {
            fn into_net_value(self) -> ValueResolver {
                ValueResolver::Dynamic(Rc::new(move || self.get().to_string()))
            }
        }
    };
}

impl_into_net_value_for_rx!(ReadSignal<T>);
impl_into_net_value_for_rx!(RwSignal<T>);
impl_into_net_value_for_rx!(Signal<T>);
impl_into_net_value_for_rx!(Memo<T>);

#[cfg(feature = "persistence")]
impl_into_net_value_for_rx!(Persistent<T>);

#[derive(Clone)]
struct CacheSpec<T> {
    #[cfg(feature = "persistence")]
    store: Cell<Option<Persistent<T>>>,
    policy: CachePolicy,
    #[cfg(not(feature = "persistence"))]
    _marker: PhantomData<T>,
}

#[derive(Clone)]
pub struct HttpClientBuilder<T, C> {
    method: HttpMethod,
    url: String,
    headers: Vec<(String, ValueResolver)>,
    query: Vec<(String, ValueResolver)>,
    path_params: Vec<(String, ValueResolver)>,
    timeout: Option<Duration>,
    body: RequestBody,
    response_codec: C,
    transport: Rc<dyn Transport>,
    cache: Option<CacheSpec<T>>,
    before_send: Vec<BeforeSendHook>,
    after_response: Vec<AfterResponseHook>,
    on_retry: Vec<OnRetryHook>,
    on_error: Vec<OnErrorHook>,
    retry: Option<crate::net::state::RetryPolicy>,
    _marker: PhantomData<T>,
}

pub struct HttpClient;

impl HttpClient {
    pub fn builder(
        method: HttpMethod,
        url: impl Into<String>,
    ) -> HttpClientBuilder<String, TextCodec> {
        HttpClientBuilder::new(method, url.into(), TextCodec)
    }

    pub fn get(url: impl Into<String>) -> HttpClientBuilder<String, TextCodec> {
        Self::builder(HttpMethod::Get, url)
    }

    pub fn post(url: impl Into<String>) -> HttpClientBuilder<String, TextCodec> {
        Self::builder(HttpMethod::Post, url)
    }

    pub fn put(url: impl Into<String>) -> HttpClientBuilder<String, TextCodec> {
        Self::builder(HttpMethod::Put, url)
    }

    pub fn patch(url: impl Into<String>) -> HttpClientBuilder<String, TextCodec> {
        Self::builder(HttpMethod::Patch, url)
    }

    pub fn delete(url: impl Into<String>) -> HttpClientBuilder<String, TextCodec> {
        Self::builder(HttpMethod::Delete, url)
    }
}

impl<T, C> HttpClientBuilder<T, C> {
    fn new(method: HttpMethod, url: String, response_codec: C) -> Self {
        Self {
            method,
            url,
            headers: Vec::new(),
            query: Vec::new(),
            path_params: Vec::new(),
            timeout: None,
            body: RequestBody::Empty,
            response_codec,
            transport: Rc::new(HttpBackend),
            cache: None,
            before_send: Vec::new(),
            after_response: Vec::new(),
            on_retry: Vec::new(),
            on_error: Vec::new(),
            retry: None,
            _marker: PhantomData,
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl IntoNetValue) -> Self {
        self.headers.push((name.into(), value.into_net_value()));
        self
    }

    pub fn query(mut self, key: impl Into<String>, value: impl IntoNetValue) -> Self {
        self.query.push((key.into(), value.into_net_value()));
        self
    }

    pub fn path_param(mut self, key: impl Into<String>, value: impl IntoNetValue) -> Self {
        self.path_params.push((key.into(), value.into_net_value()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn intercept(mut self, f: impl Fn(&mut RequestSpec) + 'static) -> Self {
        self.before_send.push(Rc::new(f));
        self
    }

    pub fn transport(mut self, transport: impl Transport) -> Self {
        self.transport = Rc::new(transport);
        self
    }

    pub fn retry(mut self, retry: crate::net::state::RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    pub fn retry_policy(self, attempts: u32, delay: Duration) -> Self {
        self.retry(crate::net::state::RetryPolicy::new(attempts, delay))
    }

    pub fn on_response(mut self, f: impl Fn(&RequestSpec, &HttpResponse) + 'static) -> Self {
        self.after_response.push(Rc::new(f));
        self
    }

    pub fn on_retry(
        mut self,
        f: impl Fn(&RequestSpec, u32, Duration, &NetError) + 'static,
    ) -> Self {
        self.on_retry.push(Rc::new(f));
        self
    }

    pub fn on_error(mut self, f: impl Fn(&RequestSpec, &NetError) + 'static) -> Self {
        self.on_error.push(Rc::new(f));
        self
    }

    pub fn text_body(mut self, value: impl Into<String>) -> Self {
        self.body = RequestBody::Text(value.into());
        self
    }

    #[cfg(feature = "json")]
    pub fn json_body<TBody>(mut self, value: TBody) -> Self
    where
        TBody: serde::Serialize,
    {
        self.body = match serde_json::to_string(&value) {
            Ok(raw) => RequestBody::Json(raw),
            Err(err) => RequestBody::Json(format!("{{\"serialize_error\":\"{}\"}}", err)),
        };
        self
    }

    pub fn form_body<I, K, V>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.body = RequestBody::Form(
            fields
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
        self
    }

    pub fn body(mut self, body: RequestBody) -> Self {
        self.body = body;
        self
    }

    pub fn cache(mut self, policy: CachePolicy) -> Self {
        if let Some(cache) = &mut self.cache {
            cache.policy = policy;
        } else {
            self.cache = Some(CacheSpec {
                policy,
                #[cfg(feature = "persistence")]
                store: Cell::new(None),
                #[cfg(not(feature = "persistence"))]
                _marker: PhantomData,
            });
        }
        self
    }

    pub fn cache_policy(self, policy: CachePolicy) -> Self {
        self.cache(policy)
    }

    pub fn text(self) -> HttpClientBuilder<String, TextCodec> {
        HttpClientBuilder {
            method: self.method,
            url: self.url,
            headers: self.headers,
            query: self.query,
            path_params: self.path_params,
            timeout: self.timeout,
            body: self.body,
            response_codec: TextCodec,
            transport: self.transport,
            cache: None,
            before_send: self.before_send,
            after_response: self.after_response,
            on_retry: self.on_retry,
            on_error: self.on_error,
            retry: self.retry,
            _marker: PhantomData,
        }
    }

    #[cfg(feature = "json")]
    pub fn json<U>(self) -> HttpClientBuilder<U, NetJsonCodec<U>>
    where
        U: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static,
    {
        HttpClientBuilder {
            method: self.method,
            url: self.url,
            headers: self.headers,
            query: self.query,
            path_params: self.path_params,
            timeout: self.timeout,
            body: self.body,
            response_codec: NetJsonCodec::new(),
            transport: self.transport,
            cache: None,
            before_send: self.before_send,
            after_response: self.after_response,
            on_retry: self.on_retry,
            on_error: self.on_error,
            retry: self.retry,
            _marker: PhantomData,
        }
    }

    fn resolve_spec(&self) -> RequestSpec {
        let mut url = self.url.clone();
        for (key, value) in &self.path_params {
            let needle = format!("{{{key}}}");
            let replacement = encode_component(&value.resolve());
            url = url.replace(&needle, &replacement);
        }

        let mut query_parts = Vec::with_capacity(self.query.len());
        for (key, value) in &self.query {
            query_parts.push(format!(
                "{}={}",
                encode_component(key),
                encode_component(&value.resolve())
            ));
        }
        if !query_parts.is_empty() {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(&query_parts.join("&"));
        }

        let headers = self
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), value.resolve()))
            .collect();

        RequestSpec {
            method: self.method,
            url,
            headers,
            timeout: self.timeout,
            body: self.body.clone(),
        }
    }

    #[cfg(feature = "persistence")]
    fn cache_key(&self, spec: &RequestSpec) -> String {
        format!("__net_cache_{}__", spec.cache_key())
    }

    #[cfg(feature = "persistence")]
    fn cached_value(&self, _spec: &RequestSpec) -> Option<T>
    where
        C: CacheCodec<T>,
        T: Clone + PartialEq + 'static,
    {
        let cache = self.cache.as_ref()?;
        if matches!(cache.policy, CachePolicy::None) {
            return None;
        }
        cache.store.get().map(|store| store.get_untracked())
    }

    #[cfg(feature = "persistence")]
    fn cache_store(&self, spec: &RequestSpec, value: T) -> Option<crate::persist::Persistent<T>>
    where
        C: CacheCodec<T>,
        T: Clone + PartialEq + 'static,
    {
        let cache = self.cache.as_ref()?;
        if matches!(cache.policy, CachePolicy::None) {
            return None;
        }
        let store = cache.store.get().unwrap_or_else(|| {
            let store = C::build_cache(self.cache_key(spec), value.clone());
            cache.store.set(Some(store));
            store
        });
        store.set(value);
        Some(store)
    }

    fn apply_interceptors(&self, spec: &mut RequestSpec) {
        for hook in &self.before_send {
            hook(spec);
        }
    }

    fn notify_error(&self, spec: &RequestSpec, err: &NetError) {
        for hook in &self.on_error {
            hook(spec, err);
        }
    }

    fn notify_retry(&self, spec: &RequestSpec, attempt: u32, delay: Duration, err: &NetError) {
        for hook in &self.on_retry {
            hook(spec, attempt, delay, err);
        }
    }

    fn notify_response(&self, spec: &RequestSpec, response: &HttpResponse) {
        for hook in &self.after_response {
            hook(spec, response);
        }
    }
}

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

            let retry = self.retry.unwrap_or(crate::net::state::RetryPolicy::new(
                1,
                Duration::from_millis(0),
            ));
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

        pub fn as_resource<S>(self, source: S) -> silex_core::reactivity::Resource<T, NetError>
        where
            S: RxGet + 'static,
            S::Value: PartialEq + RxCloneData,
        {
            silex_core::reactivity::Resource::new(source, move |_| {
                let client = self.clone();
                async move { client.send().await }
            })
        }

        pub fn as_mutation(self) -> silex_core::reactivity::Mutation<(), T, NetError> {
            silex_core::reactivity::Mutation::new(move |_| {
                let client = self.clone();
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
    T: Clone + 'static,
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

fn encode_component(value: &str) -> String {
    js_sys::encode_uri_component(value)
        .as_string()
        .unwrap_or_else(|| value.to_string())
}
