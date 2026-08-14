#[cfg(feature = "persist")]
use crate::state::CachePolicy;
use crate::{
    NetError,
    backend::{HttpBackend, Transport},
    codec::{ResponseCodec, TextCodec},
    state::{CredentialsMode, HttpMethod, HttpResponse, RequestBody, RequestSpec, RetryPolicy},
};
#[cfg(feature = "persist")]
use silex_core::CompletionOnce;
use silex_core::{ErrorReporter, Scope, SilexResult};
use std::{marker::PhantomData, rc::Rc, time::Duration};

#[cfg(any(feature = "json", feature = "persist"))]
use crate::NetErrorKind;

pub mod client_impl;
pub mod helper;
pub mod resolver;

#[cfg(feature = "persist")]
mod cache;

use helper::{base64_encode, encode_component};
pub use resolver::{IntoNetValue, ValueResolver};

#[cfg(feature = "json")]
use crate::codec::NetJsonCodec;

#[cfg(feature = "persist")]
use cache::CacheBinding;
#[cfg(feature = "persist")]
pub use cache::HttpCache;

pub type BeforeSendHook = Rc<dyn Fn(&mut RequestSpec)>;
pub type AfterResponseHook = Rc<dyn Fn(&RequestSpec, &HttpResponse)>;
pub type OnRetryHook = Rc<dyn Fn(&RequestSpec, u32, Duration, &NetError)>;
pub type OnErrorHook = Rc<dyn Fn(&RequestSpec, &NetError)>;

#[cfg(feature = "persist")]
#[derive(Clone)]
struct CacheSpec<'scope, T> {
    policy: CachePolicy,
    cache: HttpCache<'scope, T>,
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
    fn validate_runtime(&self, scope: Scope<'scope>) -> SilexResult<()> {
        match self {
            Self::Static(_) => Ok(()),
            Self::Text(value) => value.validate_runtime(scope),
            #[cfg(feature = "json")]
            Self::Json(value) => value.validate_runtime(scope),
            Self::Form(fields) => fields.iter().try_for_each(|(name, value)| {
                name.validate_runtime(scope)?;
                value.validate_runtime(scope)
            }),
        }
    }

    fn resolve(
        &self,
        resolver: fn(&ValueResolver<'scope>) -> SilexResult<String>,
    ) -> SilexResult<RequestBody> {
        match self {
            Self::Static(body) => Ok(body.clone()),
            Self::Text(value) => Ok(RequestBody::Text(resolver(value)?)),
            #[cfg(feature = "json")]
            Self::Json(value) => Ok(RequestBody::Json(resolver(value)?)),
            Self::Form(fields) => Ok(RequestBody::Form(
                fields
                    .iter()
                    .map(|(name, value)| Ok((resolver(name)?, resolver(value)?)))
                    .collect::<SilexResult<Vec<_>>>()?,
            )),
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
    pub(crate) credentials: CredentialsMode,
    body: BodyResolver<'scope>,
    pub(crate) response_codec: C,
    pub(crate) transport: Rc<dyn Transport>,
    #[cfg(feature = "persist")]
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
    pub fn builder_with_codec<'scope, T, C>(
        scope: Scope<'scope>,
        method: HttpMethod,
        url: impl IntoNetValue<'scope>,
        response_codec: C,
        error_handler: ErrorReporter<'scope>,
    ) -> HttpClientBuilder<'scope, T, C>
    where
        C: ResponseCodec<T>,
    {
        HttpClientBuilder::new(
            scope,
            method,
            url.into_net_value(),
            response_codec,
            error_handler,
        )
    }

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
            credentials: CredentialsMode::SameOrigin,
            body: BodyResolver::Static(RequestBody::Empty),
            response_codec,
            transport: Rc::new(HttpBackend),
            #[cfg(feature = "persist")]
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

    pub fn credentials(mut self, credentials: CredentialsMode) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn basic_auth(
        self,
        username: impl IntoNetValue<'scope>,
        password: impl IntoNetValue<'scope>,
    ) -> Self {
        let user = username.into_net_value();
        let password = password.into_net_value();
        let tracked_user = user.clone();
        let tracked_password = password.clone();
        let untracked_user = user;
        let untracked_password = password;
        self.header(
            "Authorization",
            ValueResolver::dynamic(
                move || {
                    let credentials = format!(
                        "{}:{}",
                        tracked_user.resolve_tracked()?,
                        tracked_password.resolve_tracked()?
                    );
                    Ok(format!("Basic {}", base64_encode(credentials.as_bytes())))
                },
                move || {
                    let credentials = format!(
                        "{}:{}",
                        untracked_user.resolve()?,
                        untracked_password.resolve()?
                    );
                    Ok(format!("Basic {}", base64_encode(credentials.as_bytes())))
                },
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
        self.header(
            "Authorization",
            ValueResolver::dynamic(
                move || Ok(format!("Bearer {}", tracked.resolve_tracked()?)),
                move || Ok(format!("Bearer {}", untracked.resolve()?)),
            ),
        )
    }

    pub fn text_body(mut self, value: impl IntoNetValue<'scope>) -> Self {
        self.body = BodyResolver::Text(value.into_net_value());
        self
    }

    #[cfg(feature = "json")]
    pub fn json_body<TBody>(mut self, value: TBody) -> Result<Self, NetError>
    where
        TBody: serde::Serialize,
    {
        let raw = serde_json::to_string(&value).map_err(|error| {
            NetError::recoverable(NetErrorKind::SerializeError(error.to_string()))
        })?;
        self.body = BodyResolver::Json(ValueResolver::static_value(raw));
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
        Ok(self)
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

    #[cfg(feature = "persist")]
    pub fn cache(
        mut self,
        policy: CachePolicy,
        cache: HttpCache<'scope, T>,
    ) -> Result<Self, crate::NetError>
    where
        T: Clone + 'static,
    {
        if !cache.belongs_to(self.scope) {
            return Err(crate::NetError::fatal(
                crate::NetErrorKind::InvalidConfiguration(
                    "HTTP cache scope does not match its builder scope".to_string(),
                ),
            ));
        }
        self.cache = Some(CacheSpec { policy, cache });
        Ok(self)
    }

    #[cfg(feature = "persist")]
    fn clear_legacy_cache_key(&self, spec: &RequestSpec) {
        let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        else {
            return;
        };
        let _ = storage.remove_item(&format!("__net_cache_{}__", spec.legacy_cache_key()));
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cached_value(&self, spec: &RequestSpec) -> Result<Option<T>, NetError>
    where
        T: Clone + 'static,
    {
        let Some(cache) = self.cache.as_ref() else {
            return Ok(None);
        };
        if matches!(cache.policy, CachePolicy::None) {
            return Ok(None);
        }
        self.clear_legacy_cache_key(spec);
        if !self.persistent_cache_allowed(spec) {
            return Ok(None);
        }
        cache.cache.cached_value(spec).map_err(NetError::from)
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cache_binding(
        &self,
        spec: &RequestSpec,
    ) -> Result<Option<CacheBinding<T>>, NetError>
    where
        T: Clone + 'static,
    {
        let Some(cache) = self.cache.as_ref() else {
            return Ok(None);
        };
        if matches!(cache.policy, CachePolicy::None) {
            return Ok(None);
        }
        self.clear_legacy_cache_key(spec);
        if !self.persistent_cache_allowed(spec) {
            return Ok(None);
        }
        cache.cache.binding(spec).map_err(NetError::from)
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cache_completion_once_for_binding(
        &self,
        binding: CacheBinding<T>,
    ) -> Result<CompletionOnce<T>, NetError>
    where
        T: Clone + 'static,
    {
        Ok(self
            .cache
            .as_ref()
            .ok_or_else(|| {
                NetError::fatal(NetErrorKind::InvalidConfiguration(
                    "cache binding requires cache configuration".to_string(),
                ))
            })?
            .cache
            .completion_once_for_binding(self.scope, binding)?)
    }

    #[cfg(feature = "persist")]
    pub(crate) fn cache_policy(&self) -> Option<CachePolicy> {
        self.cache.as_ref().map(|cache| cache.policy)
    }

    #[cfg(feature = "persist")]
    pub(crate) fn persistent_cache_allowed(&self, spec: &RequestSpec) -> bool {
        self.cache.is_some()
            && spec.is_persistent_cache_safe()
            && self.transport.supports_persistent_cache()
    }

    pub(crate) fn resolve_spec(&self) -> Result<RequestSpec, NetError> {
        self.resolve_spec_with(ValueResolver::resolve)
    }

    pub(crate) fn validate_runtime(&self) -> SilexResult<()> {
        self.url.validate_runtime(self.scope)?;
        self.headers
            .iter()
            .try_for_each(|(_, value)| value.validate_runtime(self.scope))?;
        self.query
            .iter()
            .try_for_each(|(_, value)| value.validate_runtime(self.scope))?;
        self.path_params
            .iter()
            .try_for_each(|(_, value)| value.validate_runtime(self.scope))?;
        self.body.validate_runtime(self.scope)
    }

    pub(crate) fn resolve_spec_tracked(&self) -> Result<RequestSpec, NetError> {
        self.resolve_spec_with(ValueResolver::resolve_tracked)
    }

    fn resolve_spec_with(
        &self,
        resolve: fn(&ValueResolver<'scope>) -> SilexResult<String>,
    ) -> Result<RequestSpec, NetError> {
        let mut url = resolve(&self.url).map_err(NetError::from)?;
        for (key, value) in &self.path_params {
            let needle = format!("{{{key}}}");
            url = url.replace(
                &needle,
                &encode_component(&resolve(value).map_err(NetError::from)?),
            );
        }

        let (mut url, fragment) = match url.split_once('#') {
            Some((url, fragment)) => (url.to_string(), Some(fragment)),
            None => (url, None),
        };

        let mut query_parts = Vec::with_capacity(self.query.len());
        for (key, value) in &self.query {
            query_parts.push(format!(
                "{}={}",
                encode_component(key),
                encode_component(&resolve(value).map_err(NetError::from)?)
            ));
        }
        if !query_parts.is_empty() {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(&query_parts.join("&"));
        }
        if let Some(fragment) = fragment {
            url.push('#');
            url.push_str(fragment);
        }

        let headers = self
            .headers
            .iter()
            .map(|(name, value)| Ok((name.clone(), resolve(value).map_err(NetError::from)?)))
            .collect::<Result<Vec<_>, NetError>>()?;

        Ok(RequestSpec {
            method: self.method,
            url,
            headers,
            timeout: self.timeout,
            credentials: self.credentials,
            body: self.body.resolve(resolve).map_err(NetError::from)?,
        })
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
            credentials: self.credentials,
            body: self.body,
            response_codec: TextCodec,
            transport: self.transport,
            #[cfg(feature = "persist")]
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
        U: serde::de::DeserializeOwned + Clone + 'static,
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
            credentials: self.credentials,
            body: self.body,
            response_codec: NetJsonCodec::new(),
            transport: self.transport,
            #[cfg(feature = "persist")]
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
    use silex_core::{ErrorReporter, Runtime, Scope};

    #[cfg(feature = "json")]
    use crate::NetError;
    #[cfg(feature = "json")]
    use crate::NetErrorKind;
    #[cfg(feature = "json")]
    use silex_core::reactivity::MutationState;

    #[cfg(feature = "json")]
    struct FailingSerialize;

    #[cfg(feature = "json")]
    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(<S::Error as serde::ser::Error>::custom(
                "serialization failed: \"quoted\"",
            ))
        }
    }

    fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
        scope.error_handler(|_| {}).unwrap()
    }

    #[test]
    fn request_spec_resolves_dynamic_body_and_all_request_parts() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let (url, _) = scope
                    .signal("https://example.test/{id}".to_string())
                    .unwrap();
                let (header, _) = scope.signal("token".to_string()).unwrap();
                let (query, _) = scope.signal("search".to_string()).unwrap();
                let (path, _) = scope.signal("42".to_string()).unwrap();
                let (body, _) = scope.signal("payload".to_string()).unwrap();
                let builder = HttpClient::post(scope, url, test_handler(scope))
                    .header("Authorization", header)
                    .query("q", query)
                    .path_param("id", path)
                    .text_body(body);

                assert_eq!(
                    builder.resolve_spec_tracked().unwrap().url,
                    "https://example.test/42?q=search"
                );
                assert_eq!(
                    builder.resolve_spec().unwrap().body,
                    RequestBody::Text("payload".to_string())
                );
            })
            .unwrap();
    }

    #[test]
    fn query_is_inserted_before_url_fragment() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let builder = HttpClient::get(
                    scope,
                    "https://example.test/path#fragment?not-a-query",
                    test_handler(scope),
                )
                .query("q", "one");
                assert_eq!(
                    builder.resolve_spec().unwrap().url,
                    "https://example.test/path?q=one#fragment?not-a-query"
                );

                let builder = HttpClient::get(
                    scope,
                    "https://example.test/path?existing=1#fragment",
                    test_handler(scope),
                )
                .query("q", "two");
                assert_eq!(
                    builder.resolve_spec().unwrap().url,
                    "https://example.test/path?existing=1&q=two#fragment"
                );
            })
            .unwrap();
    }

    #[test]
    fn form_body_resolves_dynamic_values_without_leaking_resolvers() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let (name, _) = scope.signal("first".to_string()).unwrap();
                let (value, set_value) = scope.signal("one".to_string()).unwrap();
                let builder = HttpClient::post(scope, "https://example.test", test_handler(scope))
                    .form_body([
                        (name.into_net_value(), value.into_net_value()),
                        (
                            ValueResolver::static_value("second"),
                            ValueResolver::static_value("two"),
                        ),
                    ]);

                assert_eq!(
                    builder.resolve_spec_tracked().unwrap().body,
                    RequestBody::Form(vec![
                        ("first".to_string(), "one".to_string()),
                        ("second".to_string(), "two".to_string()),
                    ])
                );
                set_value.set("updated".to_string()).unwrap();
                assert_eq!(
                    builder.resolve_spec().unwrap().body,
                    RequestBody::Form(vec![
                        ("first".to_string(), "updated".to_string()),
                        ("second".to_string(), "two".to_string()),
                    ])
                );
            })
            .unwrap();
    }

    #[test]
    #[cfg(feature = "json")]
    fn json_body_serialization_failure_returns_net_error() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let result = HttpClient::post(scope, "https://example.test", test_handler(scope))
                    .json_body(FailingSerialize);
                assert!(matches!(
                    result,
                    Err(NetError::Recoverable(NetErrorKind::SerializeError(message)))
                        if message.contains("serialization failed")
                ));
            })
            .unwrap();
    }

    #[cfg(feature = "json")]
    #[test]
    fn mutation_reports_json_serialization_failure_before_pending() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let mutation = HttpClient::post(scope, "https://example.test", test_handler(scope))
                    .as_mutation_with(move |_| {
                        let builder =
                            HttpClient::post(scope, "https://example.test", test_handler(scope))
                                .json_body(FailingSerialize)?;
                        Ok(builder)
                    })
                    .unwrap();

                mutation.mutate(()).unwrap();
                assert!(matches!(
                    mutation.state.get().unwrap(),
                    MutationState::Error(NetError::Recoverable(NetErrorKind::SerializeError(_)))
                ));
            })
            .unwrap();
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_body_value_contributes_dynamic_input() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let (body, _) = scope.signal("{\"value\":1}".to_string()).unwrap();
                let builder = HttpClient::post(scope, "https://example.test", test_handler(scope))
                    .json_body_value(body);

                assert_eq!(
                    builder.resolve_spec().unwrap().body,
                    RequestBody::Json("{\"value\":1}".to_string())
                );
            })
            .unwrap();
    }
}
