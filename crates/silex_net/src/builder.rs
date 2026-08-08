use crate::{
    NetError,
    backend::{HttpBackend, Transport},
    codec::TextCodec,
    state::{CachePolicy, HttpMethod, HttpResponse, RequestBody, RequestSpec, RetryPolicy},
};
#[cfg(feature = "persist")]
use silex_core::SilexResult;
use silex_core::{ErrorReporter, RuntimeInputs, Scope};
use std::{marker::PhantomData, rc::Rc, time::Duration};

pub mod client_impl;
pub mod helper;
pub mod resolver;

use helper::{base64_encode, encode_component};
pub use resolver::{IntoNetValue, ValueResolver};

#[cfg(feature = "json")]
use crate::codec::NetJsonCodec;

#[cfg(feature = "persist")]
use crate::codec::CacheCodec;
#[cfg(feature = "persist")]
use silex_persist::Persistent;
#[cfg(feature = "persist")]
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

#[cfg(feature = "persist")]
struct CacheEntry<'scope, T> {
    store: Persistent<'scope, T>,
    valid: Rc<Cell<bool>>,
}

#[cfg(feature = "persist")]
type CacheStores<'scope, T> = Rc<RefCell<Vec<(String, CacheEntry<'scope, T>)>>>;

pub type BeforeSendHook = Rc<dyn Fn(&mut RequestSpec)>;
pub type AfterResponseHook = Rc<dyn Fn(&RequestSpec, &HttpResponse)>;
pub type OnRetryHook = Rc<dyn Fn(&RequestSpec, u32, Duration, &NetError)>;
pub type OnErrorHook = Rc<dyn Fn(&RequestSpec, &NetError)>;

#[derive(Clone)]
struct CacheSpec<'scope, T> {
    #[cfg(feature = "persist")]
    policy: CachePolicy,
    _scope: PhantomData<&'scope ()>,
    #[cfg(feature = "persist")]
    default: T,
    #[cfg(feature = "persist")]
    stores: CacheStores<'scope, T>,
    #[cfg(feature = "persist")]
    generations: Rc<RefCell<HashMap<String, u64>>>,
    #[cfg(feature = "persist")]
    next_generation: Rc<Cell<u64>>,
    #[cfg(not(feature = "persist"))]
    _marker: PhantomData<T>,
}

#[cfg(feature = "persist")]
#[derive(Clone)]
pub(crate) struct CacheBinding<'scope, T> {
    pub(crate) key: String,
    pub(crate) generation: u64,
    pub(crate) store: Persistent<'scope, T>,
}

#[derive(Clone)]
enum BodyResolver<'scope> {
    Static(RequestBody),
    Text(ValueResolver<'scope>),
    #[cfg(feature = "json")]
    Json(ValueResolver<'scope>),
    Form(Vec<(ValueResolver<'scope>, ValueResolver<'scope>)>),
}

impl<'scope> BodyResolver<'scope> {
    fn inputs(&self) -> RuntimeInputs {
        let mut inputs = RuntimeInputs::new();
        match self {
            Self::Static(_) => {}
            Self::Text(value) => inputs.extend(&value.inputs()),
            #[cfg(feature = "json")]
            Self::Json(value) => inputs.extend(&value.inputs()),
            Self::Form(fields) => {
                for (name, value) in fields {
                    inputs.extend(&name.inputs());
                    inputs.extend(&value.inputs());
                }
            }
        }
        inputs
    }

    fn resolve(&self, resolver: fn(&ValueResolver<'scope>) -> String) -> RequestBody {
        match self {
            Self::Static(body) => body.clone(),
            Self::Text(value) => RequestBody::Text(resolver(value)),
            #[cfg(feature = "json")]
            Self::Json(value) => RequestBody::Json(resolver(value)),
            Self::Form(fields) => RequestBody::Form(
                fields
                    .iter()
                    .map(|(name, value)| (resolver(name), resolver(value)))
                    .collect(),
            ),
        }
    }
}

#[derive(Clone)]
pub struct HttpClientBuilder<'scope, T, C> {
    pub(crate) scope: Scope<'scope>,
    pub(crate) error_handler: ErrorReporter<'scope>,
    pub(crate) method: HttpMethod,
    pub(crate) url: ValueResolver<'scope>,
    pub(crate) headers: Vec<(String, ValueResolver<'scope>)>,
    pub(crate) query: Vec<(String, ValueResolver<'scope>)>,
    pub(crate) path_params: Vec<(String, ValueResolver<'scope>)>,
    pub(crate) timeout: Option<Duration>,
    body: BodyResolver<'scope>,
    pub(crate) response_codec: C,
    pub(crate) transport: Rc<dyn Transport>,
    cache: Option<CacheSpec<'scope, T>>,
    pub(crate) before_send: Vec<BeforeSendHook>,
    pub(crate) after_response: Vec<AfterResponseHook>,
    pub(crate) on_retry: Vec<OnRetryHook>,
    pub(crate) on_error: Vec<OnErrorHook>,
    pub(crate) retry: Option<RetryPolicy>,
    pub(crate) _marker: PhantomData<T>,
}

pub struct HttpClient;

impl HttpClient {
    pub fn builder<'scope>(
        scope: Scope<'scope>,
        method: HttpMethod,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, String, TextCodec> {
        HttpClientBuilder::new(
            scope,
            method,
            url.into_net_value(),
            TextCodec,
            error_handler,
        )
    }

    pub fn get<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, String, TextCodec> {
        Self::builder(scope, HttpMethod::Get, url, error_handler)
    }

    pub fn post<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, String, TextCodec> {
        Self::builder(scope, HttpMethod::Post, url, error_handler)
    }

    pub fn put<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, String, TextCodec> {
        Self::builder(scope, HttpMethod::Put, url, error_handler)
    }

    pub fn patch<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, String, TextCodec> {
        Self::builder(scope, HttpMethod::Patch, url, error_handler)
    }

    pub fn delete<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, String, TextCodec> {
        Self::builder(scope, HttpMethod::Delete, url, error_handler)
    }
}

impl<'scope, T, C> HttpClientBuilder<'scope, T, C> {
    fn new(
        scope: Scope<'scope>,
        method: HttpMethod,
        url: ValueResolver<'scope>,
        response_codec: C,
        error_handler: ErrorReporter<'scope>,
    ) -> Self {
        Self {
            scope,
            error_handler,
            method,
            url,
            headers: Vec::new(),
            query: Vec::new(),
            path_params: Vec::new(),
            timeout: None,
            body: BodyResolver::Static(RequestBody::Empty),
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

    pub fn header(mut self, name: impl Into<String>, value: impl IntoNetValue<'scope>) -> Self {
        let name_str = name.into();
        self.headers
            .retain(|(header, _)| !header.eq_ignore_ascii_case(&name_str));
        self.headers.push((name_str, value.into_net_value()));
        self
    }

    pub fn headers_pairs<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: IntoNetValue<'scope>,
    {
        for (key, value) in headers {
            self = self.header(key, value);
        }
        self
    }

    pub fn header_opt(
        self,
        name: impl Into<String>,
        value: Option<impl IntoNetValue<'scope>>,
    ) -> Self {
        if let Some(value) = value {
            self.header(name, value)
        } else {
            self
        }
    }

    pub fn query(mut self, key: impl Into<String>, value: impl IntoNetValue<'scope>) -> Self {
        self.query.push((key.into(), value.into_net_value()));
        self
    }

    pub fn query_pairs<I, K, V>(mut self, queries: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: IntoNetValue<'scope>,
    {
        for (key, value) in queries {
            self = self.query(key, value);
        }
        self
    }

    pub fn query_opt(
        self,
        key: impl Into<String>,
        value: Option<impl IntoNetValue<'scope>>,
    ) -> Self {
        if let Some(value) = value {
            self.query(key, value)
        } else {
            self
        }
    }

    pub fn path_param(mut self, key: impl Into<String>, value: impl IntoNetValue<'scope>) -> Self {
        self.path_params.push((key.into(), value.into_net_value()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn timeout_ms(self, millis: u64) -> Self {
        self.timeout(Duration::from_millis(millis))
    }

    pub fn basic_auth(
        self,
        username: impl IntoNetValue<'scope>,
        password: impl IntoNetValue<'scope>,
    ) -> Self {
        let user = username.into_net_value();
        let password = password.into_net_value();
        let mut inputs = user.inputs();
        inputs.extend(&password.inputs());
        let tracked_user = user.clone();
        let tracked_password = password.clone();
        let untracked_user = user;
        let untracked_password = password;
        self.header(
            "Authorization",
            ValueResolver::dynamic_with_inputs(
                move || {
                    let credentials = format!(
                        "{}:{}",
                        tracked_user.resolve_tracked(),
                        tracked_password.resolve_tracked()
                    );
                    format!("Basic {}", base64_encode(credentials.as_bytes()))
                },
                move || {
                    let credentials = format!(
                        "{}:{}",
                        untracked_user.resolve(),
                        untracked_password.resolve()
                    );
                    format!("Basic {}", base64_encode(credentials.as_bytes()))
                },
                inputs,
            ),
        )
    }

    pub fn intercept(mut self, f: impl Fn(&mut RequestSpec) + 'static) -> Self {
        self.before_send.push(Rc::new(f));
        self
    }

    pub fn transport(mut self, transport: impl Transport) -> Self {
        self.transport = Rc::new(transport);
        self
    }

    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    pub fn retry_policy(self, attempts: u32, delay: Duration) -> Self {
        self.retry(RetryPolicy::new(attempts, delay))
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

    pub fn bearer_auth(self, token: impl IntoNetValue<'scope>) -> Self {
        let token = token.into_net_value();
        let tracked = token.clone();
        let untracked = token;
        let inputs = tracked.inputs();
        self.header(
            "Authorization",
            ValueResolver::dynamic_with_inputs(
                move || format!("Bearer {}", tracked.resolve_tracked()),
                move || format!("Bearer {}", untracked.resolve()),
                inputs,
            ),
        )
    }

    pub fn text_body(mut self, value: impl IntoNetValue<'scope>) -> Self {
        self.body = BodyResolver::Text(value.into_net_value());
        self
    }

    #[cfg(feature = "json")]
    pub fn json_body<TBody>(mut self, value: TBody) -> Self
    where
        TBody: serde::Serialize,
    {
        self.body = match serde_json::to_string(&value) {
            Ok(raw) => BodyResolver::Json(ValueResolver::static_value(raw)),
            Err(error) => BodyResolver::Json(ValueResolver::static_value(format!(
                "{{\"serialize_error\":\"{}\"}}",
                error
            ))),
        };
        if !self
            .headers
            .iter()
            .any(|(header, _)| header.eq_ignore_ascii_case("content-type"))
        {
            self.headers.push((
                "Content-Type".to_string(),
                "application/json".into_net_value(),
            ));
        }
        self
    }

    #[cfg(feature = "json")]
    pub fn json_body_value(mut self, value: impl IntoNetValue<'scope>) -> Self {
        self.body = BodyResolver::Json(value.into_net_value());
        if !self
            .headers
            .iter()
            .any(|(header, _)| header.eq_ignore_ascii_case("content-type"))
        {
            self.headers.push((
                "Content-Type".to_string(),
                "application/json".into_net_value(),
            ));
        }
        self
    }

    pub fn form_body<I, K, V>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: IntoNetValue<'scope>,
        V: IntoNetValue<'scope>,
    {
        self.body = BodyResolver::Form(
            fields
                .into_iter()
                .map(|(key, value)| (key.into_net_value(), value.into_net_value()))
                .collect(),
        );
        if !self
            .headers
            .iter()
            .any(|(header, _)| header.eq_ignore_ascii_case("content-type"))
        {
            self.headers.push((
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".into_net_value(),
            ));
        }
        self
    }

    pub fn body(mut self, body: RequestBody) -> Self {
        self.body = BodyResolver::Static(body);
        self
    }

    pub fn cache_with_default(mut self, policy: CachePolicy, default: T) -> Self {
        #[cfg(not(feature = "persist"))]
        let _ = policy;
        #[cfg(feature = "persist")]
        let cache = CacheSpec {
            policy,
            _scope: PhantomData,
            default,
            stores: Rc::new(RefCell::new(Vec::new())),
            generations: Rc::new(RefCell::new(HashMap::new())),
            next_generation: Rc::new(Cell::new(0)),
        };
        #[cfg(not(feature = "persist"))]
        let cache = {
            let _ = default;
            CacheSpec {
                _scope: PhantomData,
                _marker: PhantomData,
            }
        };
        self.cache = Some(cache);
        self
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cache_key(&self, spec: &RequestSpec) -> String {
        format!("__net_cache_{}__", spec.cache_key())
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cached_value(&self, spec: &RequestSpec) -> Option<T>
    where
        C: CacheCodec<T>,
        T: Clone + PartialEq + 'static,
    {
        let cache = self.cache.as_ref()?;
        if matches!(cache.policy, CachePolicy::None) {
            return None;
        }
        let store = self.ensure_cache(spec)?;
        store.has_persisted_value().then(|| store.get_untracked())
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cache_binding(&self, spec: &RequestSpec) -> Option<CacheBinding<'scope, T>>
    where
        C: CacheCodec<T>,
        T: Clone + PartialEq + 'static,
    {
        let cache = self.cache.as_ref()?;
        if matches!(cache.policy, CachePolicy::None) {
            return None;
        }
        let key = self.cache_key(spec);
        let store = self.ensure_cache_key(&key)?;
        let generation = cache
            .next_generation
            .get()
            .checked_add(1)
            .expect("HTTP cache generation exhausted");
        cache.next_generation.set(generation);
        cache
            .generations
            .borrow_mut()
            .insert(key.clone(), generation);
        Some(CacheBinding {
            key,
            generation,
            store,
        })
    }

    #[cfg(feature = "persist")]
    pub(crate) fn ensure_cache(&self, spec: &RequestSpec) -> Option<Persistent<'scope, T>>
    where
        C: CacheCodec<T>,
        T: Clone + PartialEq + 'static,
    {
        let cache = self.cache.as_ref()?;
        if matches!(cache.policy, CachePolicy::None) {
            return None;
        }
        self.ensure_cache_key(&self.cache_key(spec))
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cache_policy(&self) -> Option<CachePolicy> {
        self.cache.as_ref().map(|cache| cache.policy)
    }

    #[cfg(feature = "persist")]
    fn ensure_cache_key(&self, key: &str) -> Option<Persistent<'scope, T>>
    where
        C: CacheCodec<T>,
        T: Clone + PartialEq + 'static,
    {
        let cache = self.cache.as_ref()?;
        if let Some(store) = cache
            .stores
            .borrow()
            .iter()
            .find(|(stored_key, entry)| stored_key == key && entry.valid.get())
            .map(|(_, entry)| entry.store)
        {
            return Some(store);
        }

        cache
            .stores
            .borrow_mut()
            .retain(|(_, entry)| entry.valid.get());
        let store = C::build_cache(
            self.scope,
            key.to_string(),
            cache.default.clone(),
            self.error_handler,
        );
        let valid = Rc::new(Cell::new(true));
        let valid_for_cleanup = valid.clone();
        if self
            .scope
            .on_cleanup(
                move || -> SilexResult<()> {
                    valid_for_cleanup.set(false);
                    Ok(())
                },
                self.error_handler,
            )
            .is_err()
        {
            return None;
        }
        cache
            .stores
            .borrow_mut()
            .push((key.to_string(), CacheEntry { store, valid }));
        Some(store)
    }

    pub(crate) fn runtime_inputs(&self) -> RuntimeInputs {
        let mut inputs = self.url.inputs();
        for (_, value) in self
            .headers
            .iter()
            .chain(self.query.iter())
            .chain(self.path_params.iter())
        {
            inputs.extend(&value.inputs());
        }
        inputs.extend(&self.body.inputs());
        inputs
    }

    pub(crate) fn validate_runtime_inputs(&self) -> Result<(), NetError> {
        self.validate_runtime_inputs_for(self.scope)
    }

    pub(crate) fn validate_runtime_inputs_for(
        &self,
        target_scope: Scope<'scope>,
    ) -> Result<(), NetError> {
        if self.scope != target_scope {
            return Err(NetError::InvalidConfiguration(
                "HTTP builder scope does not match its target scope".to_string(),
            ));
        }
        target_scope
            .try_validate_inputs(&self.runtime_inputs())
            .map_err(|error| NetError::InvalidConfiguration(error.to_string()))
    }

    pub(crate) fn resolve_spec(&self) -> RequestSpec {
        self.resolve_spec_with(ValueResolver::resolve)
    }

    pub(crate) fn resolve_spec_tracked(&self) -> RequestSpec {
        self.resolve_spec_with(ValueResolver::resolve_tracked)
    }

    fn resolve_spec_with(&self, resolve: fn(&ValueResolver<'scope>) -> String) -> RequestSpec {
        let mut url = resolve(&self.url);
        for (key, value) in &self.path_params {
            let needle = format!("{{{key}}}");
            url = url.replace(&needle, &encode_component(&resolve(value)));
        }

        let mut query_parts = Vec::with_capacity(self.query.len());
        for (key, value) in &self.query {
            query_parts.push(format!(
                "{}={}",
                encode_component(key),
                encode_component(&resolve(value))
            ));
        }
        if !query_parts.is_empty() {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(&query_parts.join("&"));
        }

        let headers = self
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), resolve(value)))
            .collect();

        RequestSpec {
            method: self.method,
            url,
            headers,
            timeout: self.timeout,
            body: self.body.resolve(resolve),
        }
    }

    pub fn text(self) -> HttpClientBuilder<'scope, String, TextCodec> {
        HttpClientBuilder {
            scope: self.scope,
            error_handler: self.error_handler,
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
    pub fn json<U>(self) -> HttpClientBuilder<'scope, U, NetJsonCodec<U>>
    where
        U: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static,
    {
        HttpClientBuilder {
            scope: self.scope,
            error_handler: self.error_handler,
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
}

#[cfg(test)]
mod tests {
    use super::{HttpClient, IntoNetValue, ValueResolver};
    use crate::state::RequestBody;
    use silex_core::{ErrorReporter, Runtime, Scope, runtime_inputs_of};

    fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
        scope.error_handler(|_| {})
    }

    #[test]
    fn request_inputs_include_dynamic_body_and_all_request_parts() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let (url, _) = scope.signal("https://example.test/{id}".to_string());
            let (header, _) = scope.signal("token".to_string());
            let (query, _) = scope.signal("search".to_string());
            let (path, _) = scope.signal("42".to_string());
            let (body, _) = scope.signal("payload".to_string());
            let builder = HttpClient::post(scope, url, test_handler(scope))
                .header("Authorization", header)
                .query("q", query)
                .path_param("id", path)
                .text_body(body);

            assert_eq!(builder.runtime_inputs().len(), 5);
            assert_eq!(
                builder.resolve_spec_tracked().url,
                "https://example.test/42?q=search"
            );
            assert_eq!(
                builder.resolve_spec().body,
                RequestBody::Text("payload".to_string())
            );
        });
    }

    #[test]
    fn form_body_resolves_dynamic_values_without_leaking_resolvers() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let (name, _) = scope.signal("first".to_string());
            let (value, set_value) = scope.signal("one".to_string());
            let builder = HttpClient::post(scope, "https://example.test", test_handler(scope))
                .form_body([
                    (name.into_net_value(), value.into_net_value()),
                    (
                        ValueResolver::static_value("second"),
                        ValueResolver::static_value("two"),
                    ),
                ]);

            assert_eq!(builder.runtime_inputs().len(), 2);
            assert_eq!(
                builder.resolve_spec_tracked().body,
                RequestBody::Form(vec![
                    ("first".to_string(), "one".to_string()),
                    ("second".to_string(), "two".to_string()),
                ])
            );
            set_value.set("updated".to_string());
            assert_eq!(
                builder.resolve_spec().body,
                RequestBody::Form(vec![
                    ("first".to_string(), "updated".to_string()),
                    ("second".to_string(), "two".to_string()),
                ])
            );
        });
    }

    #[test]
    fn foreign_dynamic_body_is_rejected_before_request_materialization() {
        let mut source_runtime = Runtime::new();
        let mut target_runtime = Runtime::new();
        source_runtime.child(|source_scope| {
            let (body, _) = source_scope.signal("foreign".to_string());
            let foreign_inputs = runtime_inputs_of(body);
            target_runtime.child(|target_scope| {
                let foreign_body = ValueResolver::dynamic_with_inputs(
                    || "foreign".to_string(),
                    || "foreign".to_string(),
                    foreign_inputs,
                );
                let builder = HttpClient::post(
                    target_scope,
                    "https://example.test",
                    test_handler(target_scope),
                )
                .text_body(foreign_body);
                assert!(builder.validate_runtime_inputs().is_err());
                assert!(builder.try_as_mutation().is_err());
            });
        });
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_body_value_contributes_dynamic_input() {
        let mut runtime = Runtime::new();
        runtime.child(|scope| {
            let (body, _) = scope.signal("{\"value\":1}".to_string());
            let builder = HttpClient::post(scope, "https://example.test", test_handler(scope))
                .json_body_value(body);

            assert_eq!(builder.runtime_inputs().len(), 1);
            assert_eq!(
                builder.resolve_spec().body,
                RequestBody::Json("{\"value\":1}".to_string())
            );
        });
    }
}
