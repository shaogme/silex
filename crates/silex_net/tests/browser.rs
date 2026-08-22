#![cfg(target_arch = "wasm32")]

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Function, Reflect};
use silex_core::reactivity::{MutationState, ResourceState};
use silex_core::{ErrorHandlerToken, OwnerAccess, Runtime, RxGet, TaskHandle};
use silex_net::{
    BrowserTransport, EventStream, EventStreamConnection, HttpMethod, HttpResponse, NetError,
    NetErrorKind, RequestBody, RequestSpec, RetryPolicy, Transport, TransportFuture, WebSocket,
    WebSocketConnection,
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope> {
    scope.error_handler(|_| {}).expect("error handler setup")
}

struct MockHost {
    global: JsValue,
    constructor_name: String,
    previous_constructor: JsValue,
    instance_name: String,
    previous_instance: JsValue,
    instances_name: String,
    previous_instances: JsValue,
}

impl MockHost {
    fn install(
        constructor_name: &str,
        instance_name: &str,
        instances_name: &str,
        body: &str,
    ) -> Self {
        let global: JsValue = js_sys::global().into();
        let constructor_key = JsValue::from_str(constructor_name);
        let instance_key = JsValue::from_str(instance_name);
        let instances_key = JsValue::from_str(instances_name);
        let previous_constructor =
            Reflect::get(&global, &constructor_key).expect("read host constructor");
        let previous_instance = Reflect::get(&global, &instance_key).expect("read host instance");
        let previous_instances =
            Reflect::get(&global, &instances_key).expect("read host instances");
        let constructor = Function::new_with_args("url", body);
        Reflect::set(&global, &constructor_key, constructor.as_ref())
            .expect("install host constructor");
        Reflect::set(&global, &instance_key, &JsValue::UNDEFINED).expect("reset host instance");
        Reflect::set(&global, &instances_key, Array::new().as_ref()).expect("reset host instances");
        Self {
            global,
            constructor_name: constructor_name.to_string(),
            previous_constructor,
            instance_name: instance_name.to_string(),
            previous_instance,
            instances_name: instances_name.to_string(),
            previous_instances,
        }
    }

    fn websocket() -> Self {
        Self::install(
            "WebSocket",
            "__silex_test_socket",
            "__silex_test_socket_instances",
            r#"
                if (url === "mock://failure") {
                    throw new TypeError("constructor failure");
                }
                const socket = {
                    url: url,
                    readyState: 0,
                    sent: [],
                    closeCalls: 0,
                    onopen: null,
                    onmessage: null,
                     onerror: null,
                     onclose: null,
                     send: function (data) {
                         if (this.readyState === 0) {
                             throw new Error("InvalidStateError");
                         }
                         if (this.readyState === 2 || this.readyState === 3) {
                             return;
                         }
                         this.sent.push(data);
                     },
                     close: function () {
                         this.closeCalls += 1;
                         this.readyState = 2;
                     },
                    emitOpen: function () {
                        this.readyState = 1;
                        if (this.onopen) {
                            this.onopen(new Event("open"));
                        }
                    },
                    emitMessage: function (data) {
                        if (this.onmessage) {
                            this.onmessage(new MessageEvent("message", { data: String(data) }));
                        }
                    },
                    emitError: function (message) {
                        if (this.onerror) {
                            this.onerror(new ErrorEvent("error", { message: String(message) }));
                        }
                    },
                    emitClose: function () {
                        this.readyState = 3;
                        const callback = this.onclose;
                        if (callback) {
                            const event = new CloseEvent("close", {
                                code: 1006,
                                reason: "remote"
                            });
                            callback(event);
                            if (this.onclose) {
                                this.onclose(event);
                            }
                        }
                    }
                };
                globalThis.__silex_test_socket = socket;
                globalThis.__silex_test_socket_instances.push(socket);
                return socket;
            "#,
        )
    }

    fn event_source() -> Self {
        Self::install(
            "EventSource",
            "__silex_test_event_source",
            "__silex_test_event_source_instances",
            r#"
                if (url === "mock://failure") {
                    throw new TypeError("constructor failure");
                }
                const listeners = Object.create(null);
                const source = {
                    url: url,
                    readyState: 0,
                    closeCalls: 0,
                    removeCalls: 0,
                    onopen: null,
                    onmessage: null,
                    onerror: null,
                    addEventListener: function (name, callback) {
                        const list = listeners[name] || [];
                        list.push(callback);
                        listeners[name] = list;
                    },
                    removeEventListener: function (name, callback) {
                        const list = listeners[name] || [];
                        listeners[name] = list.filter(function (item) {
                            return item !== callback;
                        });
                        this.removeCalls += 1;
                    },
                    close: function () {
                        this.closeCalls += 1;
                        this.readyState = 2;
                    },
                    emitOpen: function () {
                        this.readyState = 1;
                        if (this.onopen) {
                            this.onopen(new Event("open"));
                        }
                    },
                    emitMessage: function (data) {
                        if (this.onmessage) {
                            this.onmessage(new MessageEvent("message", { data: String(data) }));
                        }
                    },
                    emitNamed: function (name, data) {
                        const list = (listeners[name] || []).slice();
                        const event = new MessageEvent(name, { data: String(data) });
                        list.forEach(function (callback) {
                            callback(event);
                        });
                    },
                    emitError: function () {
                        if (this.onerror) {
                            this.onerror(new Event("error"));
                        }
                    }
                };
                globalThis.__silex_test_event_source = source;
                globalThis.__silex_test_event_source_instances.push(source);
                return source;
            "#,
        )
    }
}

impl Drop for MockHost {
    fn drop(&mut self) {
        let constructor_key = JsValue::from_str(&self.constructor_name);
        let instance_key = JsValue::from_str(&self.instance_name);
        let instances_key = JsValue::from_str(&self.instances_name);
        let _ = Reflect::set(&self.global, &constructor_key, &self.previous_constructor);
        let _ = Reflect::set(&self.global, &instance_key, &self.previous_instance);
        let _ = Reflect::set(&self.global, &instances_key, &self.previous_instances);
    }
}

fn mock_object(name: &str) -> JsValue {
    let global: JsValue = js_sys::global().into();
    Reflect::get(&global, &JsValue::from_str(name)).expect("read mock object")
}

fn mock_call0(object_name: &str, method_name: &str) {
    let object = mock_object(object_name);
    let method = Reflect::get(&object, &JsValue::from_str(method_name))
        .expect("read mock method")
        .dyn_into::<Function>()
        .expect("mock method function");
    method.call0(&object).expect("call mock method");
}

fn mock_call1(object_name: &str, method_name: &str, value: &str) {
    let object = mock_object(object_name);
    let method = Reflect::get(&object, &JsValue::from_str(method_name))
        .expect("read mock method")
        .dyn_into::<Function>()
        .expect("mock method function");
    method
        .call1(&object, &JsValue::from_str(value))
        .expect("call mock method");
}

fn mock_call2(object_name: &str, method_name: &str, first: &str, second: &str) {
    let object = mock_object(object_name);
    let method = Reflect::get(&object, &JsValue::from_str(method_name))
        .expect("read mock method")
        .dyn_into::<Function>()
        .expect("mock method function");
    method
        .call2(
            &object,
            &JsValue::from_str(first),
            &JsValue::from_str(second),
        )
        .expect("call mock method");
}

fn mock_property(object_name: &str, property_name: &str) -> JsValue {
    let object = mock_object(object_name);
    Reflect::get(&object, &JsValue::from_str(property_name)).expect("read mock property")
}

fn mock_property_is_cleared(object_name: &str, property_name: &str) -> bool {
    let value = mock_property(object_name, property_name);
    value.is_null() || value.is_undefined()
}

fn mock_instance_count(instances_name: &str) -> usize {
    mock_object(instances_name)
        .dyn_into::<Array>()
        .expect("mock instances array")
        .length() as usize
}

struct PendingFuture {
    dropped: Rc<Cell<usize>>,
}

impl Future for PendingFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for PendingFuture {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

#[derive(Clone)]
struct ScriptedTransport {
    calls: Rc<Cell<usize>>,
    status: u16,
    body: &'static str,
    delay_ms: u32,
}

impl Transport for ScriptedTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        self.calls.set(self.calls.get() + 1);
        let response = HttpResponse {
            url: spec.url,
            status: self.status,
            status_text: String::new(),
            raw_body: self.body.to_string(),
        };
        let delay_ms = self.delay_ms;
        Box::pin(async move {
            if delay_ms > 0 {
                TimeoutFuture::new(delay_ms).await;
            }
            Ok(response)
        })
    }

    #[cfg(feature = "persist")]
    fn supports_persistent_cache(&self) -> bool {
        true
    }
}

#[derive(Clone)]
struct RecordingTransport {
    urls: Rc<RefCell<Vec<String>>>,
}

impl Transport for RecordingTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        self.urls.borrow_mut().push(spec.url.clone());
        let response = HttpResponse {
            url: spec.url,
            status: 200,
            status_text: String::new(),
            raw_body: "ok".to_string(),
        };
        Box::pin(async move { Ok(response) })
    }
}

#[derive(Clone)]
struct ReplacementTransport {
    calls: Rc<Cell<usize>>,
}

impl Transport for ReplacementTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        self.calls.set(self.calls.get() + 1);
        let first = spec.url.ends_with("value=first");
        let response = HttpResponse {
            url: spec.url,
            status: 200,
            status_text: String::new(),
            raw_body: if first {
                "first".to_string()
            } else {
                "second".to_string()
            },
        };
        Box::pin(async move {
            if first {
                TimeoutFuture::new(20).await;
            }
            Ok(response)
        })
    }
}

#[derive(Clone)]
#[cfg(feature = "persist")]
struct GenerationTransport {
    calls: Rc<Cell<usize>>,
}

#[cfg(feature = "persist")]
impl Transport for GenerationTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        let response = HttpResponse {
            url: spec.url,
            status: 200,
            status_text: String::new(),
            raw_body: if call == 1 {
                "stale".to_string()
            } else {
                "fresh".to_string()
            },
        };
        Box::pin(async move {
            if call == 1 {
                TimeoutFuture::new(20).await;
            }
            Ok(response)
        })
    }

    fn supports_persistent_cache(&self) -> bool {
        true
    }
}

#[derive(Clone)]
struct MutationTransport {
    calls: Rc<Cell<usize>>,
}

impl Transport for MutationTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        self.calls.set(self.calls.get() + 1);
        let first = spec.url.ends_with("id=1");
        let response = HttpResponse {
            url: spec.url,
            status: 200,
            status_text: String::new(),
            raw_body: if first {
                "one".to_string()
            } else {
                "two".to_string()
            },
        };
        Box::pin(async move {
            if first {
                TimeoutFuture::new(20).await;
            }
            Ok(response)
        })
    }
}

#[wasm_bindgen_test(async)]
async fn browser_transport_fetches_data_url() {
    let response = BrowserTransport::send(RequestSpec {
        method: HttpMethod::Get,
        url: "data:text/plain,hello".to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::SameOrigin,
        timeout: None,
        body: RequestBody::Empty,
    })
    .await
    .expect("data URL fetch should succeed");

    assert_eq!(response.raw_body, "hello");
}

#[wasm_bindgen_test(async)]
async fn http_resource_resolves_owned_request() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (source, _) = scope.signal(1_u32).unwrap();
        let resource =
            silex_net::HttpClient::get(scope, "data:text/plain,hello", test_handler(scope))
                .as_resource(source, None)
                .expect("resource setup");
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "hello"
        ));
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn transport_receives_query_before_url_fragment() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let urls = Rc::new(RefCell::new(Vec::new()));
        let result = silex_net::HttpClient::get(
            scope,
            "https://example.test/path#fragment?not-a-query",
            test_handler(scope),
        )
        .query("q", "one")
        .transport(RecordingTransport { urls: urls.clone() })
        .send()
        .await
        .expect("recording transport should succeed");

        assert_eq!(result, "ok");
        assert_eq!(
            urls.borrow().as_slice(),
            ["https://example.test/path?q=one#fragment?not-a-query"]
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn resource_runs_interceptor_once_and_rejects_custom_status() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (source, _) = scope.signal(1_u32).unwrap();
        let interceptor_calls = Rc::new(Cell::new(0));
        let transport_calls = Rc::new(Cell::new(0));
        let interceptor_calls_for_hook = interceptor_calls.clone();
        let resource =
            silex_net::HttpClient::get(scope, "https://example.test/success", test_handler(scope))
                .intercept(move |_| {
                    interceptor_calls_for_hook.set(interceptor_calls_for_hook.get() + 1);
                })
                .transport(ScriptedTransport {
                    calls: transport_calls.clone(),
                    status: 200,
                    body: "ok",
                    delay_ms: 0,
                })
                .as_resource(source, None)
                .expect("resource setup");
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "ok"
        ));
        assert_eq!(interceptor_calls.get(), 1);
        assert_eq!(transport_calls.get(), 1);
    }
    .await;
    root.close().expect("root cleanup");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (source, _) = scope.signal(1_u32).unwrap();
        let transport_calls = Rc::new(Cell::new(0));
        let error_calls = Rc::new(Cell::new(0));
        let retry_calls = Rc::new(Cell::new(0));
        let error_calls_for_hook = error_calls.clone();
        let retry_calls_for_hook = retry_calls.clone();
        let resource =
            silex_net::HttpClient::get(scope, "https://example.test/status", test_handler(scope))
                .transport(ScriptedTransport {
                    calls: transport_calls.clone(),
                    status: 503,
                    body: "unavailable",
                    delay_ms: 0,
                })
                .retry(RetryPolicy::new(3, std::time::Duration::ZERO).no_jitter())
                .on_error(move |_, error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::HttpStatus { status: 503, .. })
                    ));
                    error_calls_for_hook.set(error_calls_for_hook.get() + 1);
                })
                .on_retry(move |_, _, _, error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::HttpStatus { status: 503, .. })
                    ));
                    retry_calls_for_hook.set(retry_calls_for_hook.get() + 1);
                })
                .as_resource(source, None)
                .expect("resource setup");
        TimeoutFuture::new(1).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Error(NetError::Recoverable(NetErrorKind::HttpStatus {
                status: 503,
                ..
            }))
        ));
        assert_eq!(transport_calls.get(), 4);
        assert_eq!(error_calls.get(), 4);
        assert_eq!(retry_calls.get(), 3);
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn resource_replacement_keeps_new_request_result() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (query, set_query) = scope.signal("first".to_string()).unwrap();
        let (source, _) = scope.signal(1_u32).unwrap();
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(
            scope,
            "https://example.test/replacement",
            test_handler(scope),
        )
        .query("value", query)
        .transport(ReplacementTransport {
            calls: calls.clone(),
        })
        .as_resource(source, None)
        .expect("resource setup");
        TimeoutFuture::new(0).await;
        assert_eq!(calls.get(), 1);
        set_query.set("second".to_string()).unwrap();
        TimeoutFuture::new(30).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "second"
        ));
        assert_eq!(calls.get(), 2);
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn mutation_preflight_error_does_not_enter_pending() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.owner().expect("source runtime setup");
    let target_root = target_runtime.owner().expect("target runtime setup");
    let source_scope = source_root.access();
    let target_scope = target_root.access();
    let (foreign, _) = source_scope.signal("foreign".to_string()).unwrap();
    let mutation = silex_net::HttpClient::post(
        target_scope,
        "https://example.test/mutate",
        test_handler(target_scope),
    )
    .as_mutation_with(move |_| {
        Ok(silex_net::HttpClient::post(
            target_scope,
            "https://example.test/mutate",
            test_handler(target_scope),
        )
        .text_body(foreign))
    })
    .expect("mutation setup");
    mutation.mutate(()).unwrap();
    assert!(matches!(
        mutation.state.get().unwrap(),
        MutationState::Error(NetError::Fatal(NetErrorKind::InvalidConfiguration(_)))
    ));
    source_root.close().expect("source root cleanup");
    target_root.close().expect("target root cleanup");
}

#[wasm_bindgen_test(async)]
async fn mutation_commits_only_the_latest_completion() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let calls = Rc::new(Cell::new(0));
        let mutation =
            silex_net::HttpClient::get(scope, "https://example.test/mutation", test_handler(scope))
                .as_mutation_with({
                    let calls = calls.clone();
                    move |id: u32| {
                        Ok(silex_net::HttpClient::get(
                            scope,
                            "https://example.test/mutation",
                            test_handler(scope),
                        )
                        .query("id", id)
                        .transport(MutationTransport {
                            calls: calls.clone(),
                        }))
                    }
                })
                .expect("mutation setup");
        mutation.mutate(1).unwrap();
        mutation.mutate(2).unwrap();
        assert!(matches!(
            mutation.state.get().unwrap(),
            MutationState::Pending
        ));
        TimeoutFuture::new(30).await;
        assert!(matches!(
            mutation.state.get().unwrap(),
            MutationState::Success(value) if value == "two"
        ));
        assert_eq!(calls.get(), 2);
    }
    .await;
    root.close().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn cache_first_does_not_treat_default_as_history() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (source, _) = scope.signal(1_u32).unwrap();
        let resource =
            silex_net::HttpClient::get(scope, "data:text/plain,cache", test_handler(scope))
                .credentials(silex_net::CredentialsMode::Omit)
                .cache(
                    silex_net::CachePolicy::CacheFirst,
                    silex_net::HttpCache::new(
                        scope,
                        silex_net::CacheConfig::default(),
                        silex_net::TextCodec,
                    )
                    .expect("cache setup"),
                )
                .expect("cache policy setup")
                .as_resource(source, None)
                .expect("resource setup");
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "cache"
        ));
    }
    .await;
    root.close().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn cache_controller_bounds_dynamic_keys_and_removes_evicted_history() {
    let url = "https://example.test/bounded-cache";
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    let storage_key = |value: &str| {
        let spec = RequestSpec {
            method: HttpMethod::Get,
            url: format!("{url}?value={value}"),
            headers: Vec::new(),
            credentials: silex_net::CredentialsMode::Omit,
            timeout: None,
            body: RequestBody::Empty,
        };
        format!("__net_cache_{}__", spec.cache_key())
    };
    let first_key = storage_key("first");
    let second_key = storage_key("second");
    let third_key = storage_key("third");
    for key in [&first_key, &second_key, &third_key] {
        storage.remove_item(key).expect("clear cache key");
    }

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let storage_for_scope = storage.clone();
    let first_key_for_scope = first_key.clone();
    let second_key_for_scope = second_key.clone();
    let third_key_for_scope = third_key.clone();
    let scope = root.access();
    async move {
        let (query, set_query) = scope.signal(String::new()).unwrap();
        let calls = Rc::new(Cell::new(0));
        let client = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .query("value", query)
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default().capacity(2),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "network",
                delay_ms: 0,
            });

        for value in ["first", "second", "third"] {
            set_query.set(value.to_string()).unwrap();
            assert_eq!(
                client.send().await.expect("bounded cache request"),
                "network"
            );
        }
        assert_eq!(calls.get(), 3);
        assert!(
            storage_for_scope
                .get_item(&first_key_for_scope)
                .expect("read evicted key")
                .is_none()
        );
        assert_eq!(
            storage_for_scope
                .get_item(&second_key_for_scope)
                .expect("read second key"),
            Some("network".to_string())
        );
        assert_eq!(
            storage_for_scope
                .get_item(&third_key_for_scope)
                .expect("read third key"),
            Some("network".to_string())
        );
    }
    .await;
    root.close().expect("root cleanup");

    assert!(
        storage
            .get_item(&first_key)
            .expect("read first key")
            .is_none()
    );
    storage
        .remove_item(&second_key)
        .expect("cleanup second key");
    storage.remove_item(&third_key).expect("cleanup third key");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn cache_controller_expires_scope_entries_with_ttl() {
    let url = "https://example.test/ttl-cache";
    let spec = RequestSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage.remove_item(&storage_key).expect("clear ttl key");
    storage
        .set_item(&storage_key, "history")
        .expect("seed ttl history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let storage_for_scope = storage.clone();
    let storage_key_for_scope = storage_key.clone();
    let scope = root.access();
    async move {
        let calls = Rc::new(Cell::new(0));
        let client = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default()
                        .capacity(1)
                        .ttl(std::time::Duration::ZERO),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "network",
                delay_ms: 0,
            });

        assert_eq!(client.send().await.expect("history request"), "history");
        assert_eq!(calls.get(), 0);
        assert_eq!(client.send().await.expect("expired request"), "network");
        assert_eq!(calls.get(), 1);
        assert_eq!(
            storage_for_scope
                .get_item(&storage_key_for_scope)
                .expect("read refreshed ttl key"),
            Some("network".to_string())
        );
    }
    .await;
    root.close().expect("root cleanup");
    storage.remove_item(&storage_key).expect("cleanup ttl key");
}

#[cfg(all(feature = "json", feature = "persist"))]
#[wasm_bindgen_test(async)]
async fn json_cache_codec_round_trips_persisted_values() {
    let url = "https://example.test/json-cache";
    let spec = RequestSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage.remove_item(&storage_key).expect("clear json key");
    storage
        .set_item(&storage_key, "7")
        .expect("seed json history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let calls = Rc::new(Cell::new(0));
        let result = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .credentials(silex_net::CredentialsMode::Omit)
            .json::<u32>()
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::NetJsonCodec::<u32>::new(),
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "42",
                delay_ms: 0,
            })
            .send()
            .await
            .expect("json cache request");
        assert_eq!(result, 7);
        assert_eq!(calls.get(), 0);
    }
    .await;
    root.close().expect("root cleanup");
    storage.remove_item(&storage_key).expect("cleanup json key");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn cache_completion_cannot_recreate_an_evicted_key() {
    let url = "https://example.test/evicted-completion";
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    let storage_key = |value: &str| {
        let spec = RequestSpec {
            method: HttpMethod::Get,
            url: format!("{url}?value={value}"),
            headers: Vec::new(),
            credentials: silex_net::CredentialsMode::Omit,
            timeout: None,
            body: RequestBody::Empty,
        };
        format!("__net_cache_{}__", spec.cache_key())
    };
    let first_key = storage_key("first");
    let second_key = storage_key("second");
    storage.remove_item(&first_key).expect("clear first key");
    storage.remove_item(&second_key).expect("clear second key");
    storage
        .set_item(&first_key, "history")
        .expect("seed first history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let storage_for_scope = storage.clone();
    let first_key_for_scope = first_key.clone();
    let second_key_for_scope = second_key.clone();
    let scope = root.access();
    async move {
        let (query, set_query) = scope.signal("first".to_string()).unwrap();
        let (source, _) = scope.signal(1_u32).unwrap();
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .query("value", query)
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::StaleWhileRevalidate,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default().capacity(1),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(GenerationTransport {
                calls: calls.clone(),
            })
            .as_resource(source, None)
            .expect("resource setup");
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 1);

        set_query.set("second".to_string()).unwrap();
        TimeoutFuture::new(30).await;
        assert_eq!(
            resource.state().get().unwrap(),
            ResourceState::Ready("fresh".to_string())
        );
        assert_eq!(calls.get(), 2);
        assert!(
            storage_for_scope
                .get_item(&first_key_for_scope)
                .expect("read evicted completion key")
                .is_none()
        );
        assert_eq!(
            storage_for_scope
                .get_item(&second_key_for_scope)
                .expect("read current completion key"),
            Some("fresh".to_string())
        );
    }
    .await;
    root.close().expect("root cleanup");
    storage.remove_item(&first_key).expect("cleanup first key");
    storage
        .remove_item(&second_key)
        .expect("cleanup second key");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn credentialed_request_skips_persistent_history() {
    let url = "https://example.test/credentialed-cache";
    let spec = RequestSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::SameOrigin,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage
        .set_item(&storage_key, "history")
        .expect("seed cache history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let calls = Rc::new(Cell::new(0));
        let result = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "network",
                delay_ms: 0,
            })
            .send()
            .await
            .expect("credentialed request should use network");
        assert_eq!(result, "network");
        assert_eq!(calls.get(), 1);
    }
    .await;
    root.close().expect("root cleanup");
    storage
        .remove_item(&storage_key)
        .expect("remove cache history");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn non_idempotent_request_does_not_create_persistent_cache() {
    let url = "https://example.test/non-idempotent-cache";
    let spec = RequestSpec {
        method: HttpMethod::Post,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage
        .remove_item(&storage_key)
        .expect("clear cache history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let calls = Rc::new(Cell::new(0));
        let result = silex_net::HttpClient::post(scope, url, test_handler(scope))
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "network",
                delay_ms: 0,
            })
            .send()
            .await
            .expect("non-idempotent request should use network");
        assert_eq!(result, "network");
        assert_eq!(calls.get(), 1);
    }
    .await;
    root.close().expect("root cleanup");
    assert!(
        storage
            .get_item(&storage_key)
            .expect("read cache history")
            .is_none()
    );
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn custom_transport_must_opt_into_persistent_cache() {
    let url = "https://example.test/custom-transport-cache";
    let spec = RequestSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage
        .set_item(&storage_key, "history")
        .expect("seed cache history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let calls = Rc::new(Cell::new(0));
        let result = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(MutationTransport {
                calls: calls.clone(),
            })
            .send()
            .await
            .expect("untrusted transport should use network");
        assert_eq!(result, "two");
        assert_eq!(calls.get(), 1);
    }
    .await;
    root.close().expect("root cleanup");
    storage
        .remove_item(&storage_key)
        .expect("remove cache history");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn cache_first_reloads_history_when_request_key_changes() {
    let url = "https://example.test/cache-key";
    let first_spec = RequestSpec {
        method: HttpMethod::Get,
        url: format!("{url}?value=first"),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", first_spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage
        .set_item(&storage_key, "history")
        .expect("seed cache history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (query, set_query) = scope.signal("first".to_string()).unwrap();
        let (source, _) = scope.signal(1_u32).unwrap();
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .query("value", query)
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::CacheFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "network",
                delay_ms: 0,
            })
            .as_resource(source, None)
            .expect("resource setup");
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 0);

        set_query.set("second".to_string()).unwrap();
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "network"
        ));
        assert_eq!(calls.get(), 1);

        set_query.set("first".to_string()).unwrap();
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 1);
    }
    .await;
    root.close().expect("root cleanup");
    storage
        .remove_item(&storage_key)
        .expect("remove cache history");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn swr_rejects_stale_same_key_cache_write() {
    let url = "https://example.test/swr-generation";
    let spec = RequestSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage
        .set_item(&storage_key, "history")
        .expect("seed cache history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (source, set_source) = scope.signal(1_u32).unwrap();
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::StaleWhileRevalidate,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(GenerationTransport {
                calls: calls.clone(),
            })
            .as_resource(source, None)
            .expect("resource setup");
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 1);

        set_source.set(2).unwrap();
        TimeoutFuture::new(30).await;
        let state = resource.state().get().unwrap();
        assert_eq!(
            state,
            ResourceState::Ready("fresh".to_string()),
            "state after SWR generation test, transport calls={}",
            calls.get()
        );
        assert_eq!(calls.get(), 2);
    }
    .await;
    root.close().expect("root cleanup");
    assert_eq!(
        storage
            .get_item(&storage_key)
            .expect("read cache history")
            .as_deref(),
        Some("fresh")
    );
    storage
        .remove_item(&storage_key)
        .expect("remove cache history");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn network_first_uses_history_after_retryable_failure() {
    let url = "https://example.test/network-first";
    let spec = RequestSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: Vec::new(),
        credentials: silex_net::CredentialsMode::Omit,
        timeout: None,
        body: RequestBody::Empty,
    };
    let storage_key = format!("__net_cache_{}__", spec.cache_key());
    let storage = web_sys::window()
        .expect("browser window")
        .local_storage()
        .expect("localStorage access")
        .expect("localStorage");
    storage
        .set_item(&storage_key, "history")
        .expect("seed cache history");

    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let (source, _) = scope.signal(1_u32).unwrap();
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .credentials(silex_net::CredentialsMode::Omit)
            .cache(
                silex_net::CachePolicy::NetworkFirst,
                silex_net::HttpCache::new(
                    scope,
                    silex_net::CacheConfig::default(),
                    silex_net::TextCodec,
                )
                .expect("cache setup"),
            )
            .expect("cache policy setup")
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 503,
                body: "unavailable",
                delay_ms: 0,
            })
            .as_resource(source, None)
            .expect("resource setup");
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state().get().unwrap(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 2);
    }
    .await;
    root.close().expect("root cleanup");
    storage
        .remove_item(&storage_key)
        .expect("remove cache history");
}

#[wasm_bindgen_test(async)]
async fn task_cancel_drops_pending_scoped_future() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let dropped_for_scope = dropped.clone();
    let scope = root.access();
    async move {
        let task: TaskHandle<'_> = scope
            .spawn_scoped(
                PendingFuture {
                    dropped: dropped_for_scope.clone(),
                },
                test_handler(scope),
            )
            .expect("scoped task setup");
        TimeoutFuture::new(10).await;
        assert_eq!(dropped_for_scope.get(), 0);
        task.cancel();
        TimeoutFuture::new(0).await;
        assert_eq!(dropped_for_scope.get(), 1);
    }
    .await;
    root.close().expect("root cleanup");
    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test]
fn lazy_connections_validate_scope_without_opening_host_resources() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let socket = WebSocket::lazy(scope, "wss://example.test", test_handler(scope))
                .build()
                .expect("websocket setup");
            let stream =
                EventStream::lazy(scope, "https://example.test/events", test_handler(scope))
                    .build()
                    .expect("event stream setup");
            assert!(!socket.state().get().unwrap().is_active());
            assert!(!stream.state().get().unwrap().is_active());
        })
        .unwrap();
}

#[wasm_bindgen_test(async)]
async fn websocket_host_bridge_covers_events_retry_and_manual_close() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let opened = Rc::new(Cell::new(0));
        let errors = Rc::new(Cell::new(0));
        let closed = Rc::new(Cell::new(0));
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .reconnect(3, std::time::Duration::ZERO)
            .on_open({
                let opened = opened.clone();
                move || opened.set(opened.get() + 1)
            })
            .on_error({
                let errors = errors.clone();
                move |error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::JsError(message))
                            if message == "broken"
                    ));
                    errors.set(errors.get() + 1);
                }
            })
            .on_close({
                let closed = closed.clone();
                move |_, _| closed.set(closed.get() + 1)
            })
            .build()
            .expect("websocket setup");

        socket.reconnect().expect("websocket reconnect");
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 1);
        assert_eq!(
            socket.send_text("too-early"),
            Err(NetError::Recoverable(NetErrorKind::ConnectionNotReady {
                state: silex_net::ConnectionState::Connecting
            }))
        );
        mock_call0("__silex_test_socket", "emitOpen");
        TimeoutFuture::new(0).await;
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Connected
        );
        assert_eq!(opened.get(), 1);

        mock_call1("__silex_test_socket", "emitMessage", "first");
        assert_eq!(
            socket.raw_message().get().unwrap().as_deref(),
            Some("first")
        );
        socket.send_text("outbound").expect("send on mock socket");
        let sent = mock_property("__silex_test_socket", "sent")
            .dyn_into::<Array>()
            .expect("sent array");
        assert_eq!(sent.length(), 1);
        assert_eq!(sent.get(0).as_string().as_deref(), Some("outbound"));

        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(closed.get(), 1);
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Closed
        );
        TimeoutFuture::new(0).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 2);
        mock_call0("__silex_test_socket", "emitOpen");
        assert_eq!(opened.get(), 2);

        mock_call1("__silex_test_socket", "emitError", "broken");
        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(errors.get(), 1);
        assert!(matches!(
            socket.error().get().unwrap(),
            Some(NetError::Recoverable(NetErrorKind::JsError(message)))
                if message == "broken"
        ));
        assert_eq!(closed.get(), 2);
        TimeoutFuture::new(0).await;
        assert_eq!(
            mock_instance_count("__silex_test_socket_instances"),
            3,
            "error and close must schedule one retry"
        );
        mock_call0("__silex_test_socket", "emitOpen");
        assert_eq!(opened.get(), 3);
        assert_eq!(socket.error().get().unwrap(), None);

        socket.close().expect("manual close");
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Closing
        );
        assert_eq!(
            socket.send_text("during-close"),
            Err(NetError::Recoverable(NetErrorKind::ConnectionNotReady {
                state: silex_net::ConnectionState::Closing
            }))
        );
        assert!(!mock_property_is_cleared("__silex_test_socket", "onclose"));
        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(closed.get(), 3);
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Closed
        );
        assert_eq!(
            socket.send_text("after-close"),
            Err(NetError::Recoverable(NetErrorKind::ConnectionClosed))
        );
        assert!(mock_property_is_cleared("__silex_test_socket", "onopen"));
        assert!(mock_property_is_cleared("__silex_test_socket", "onmessage"));
        assert!(mock_property_is_cleared("__silex_test_socket", "onerror"));
        assert!(mock_property_is_cleared("__silex_test_socket", "onclose"));
        mock_call1("__silex_test_socket", "emitMessage", "late");
        assert_eq!(
            socket.raw_message().get().unwrap().as_deref(),
            Some("first")
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_retry_window_counts_continuous_pre_open_failures() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .reconnect_policy(RetryPolicy::new(3, std::time::Duration::ZERO).no_jitter())
            .build()
            .expect("websocket setup");

        socket.reconnect().expect("websocket reconnect");
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 1);

        mock_call0("__silex_test_socket", "emitClose");
        TimeoutFuture::new(0).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 2);

        mock_call0("__silex_test_socket", "emitClose");
        TimeoutFuture::new(0).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 3);

        mock_call0("__silex_test_socket", "emitClose");
        TimeoutFuture::new(0).await;
        assert_eq!(
            mock_instance_count("__silex_test_socket_instances"),
            4,
            "three retries are allowed after the initial connection"
        );

        mock_call0("__silex_test_socket", "emitClose");
        TimeoutFuture::new(0).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 4);

        socket.reconnect().expect("websocket reconnect");
        assert_eq!(
            mock_instance_count("__silex_test_socket_instances"),
            5,
            "manual reconnect must start a fresh retry window"
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_callbacks_can_control_connection_after_state_restore() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let connection_slot: Rc<Cell<Option<WebSocketConnection<'_>>>> = Rc::new(Cell::new(None));
        let send_succeeded = Rc::new(Cell::new(false));
        let connection_for_open = connection_slot.clone();
        let send_succeeded_for_open = send_succeeded.clone();
        let connection_for_close = connection_slot.clone();
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .on_open(move || {
                let socket = connection_for_open
                    .get()
                    .expect("connection must be available in on_open");
                socket
                    .send_text("from callback")
                    .expect("send from on_open callback");
                send_succeeded_for_open.set(true);
            })
            .on_close(move |_, _| {
                connection_for_close
                    .get()
                    .expect("connection must be available in on_close")
                    .reconnect()
                    .expect("websocket reconnect from close callback");
            })
            .build()
            .expect("websocket setup");
        connection_slot.set(Some(socket));

        socket.reconnect().expect("websocket reconnect");
        mock_call0("__silex_test_socket", "emitOpen");
        TimeoutFuture::new(0).await;
        assert!(send_succeeded.get());

        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 2);
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Connecting
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_constructor_failure_reports_error_before_connection_creation() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let errors = Rc::new(Cell::new(0));
        let result = WebSocket::connect(scope, "mock://failure", test_handler(scope))
            .on_error({
                let errors = errors.clone();
                move |error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::JsError(_))
                    ));
                    errors.set(errors.get() + 1);
                }
            })
            .build();
        assert!(matches!(
            result,
            Err(NetError::Recoverable(NetErrorKind::JsError(_)))
        ));
        assert_eq!(errors.get(), 1);
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 0);

        let (url, set_url) = scope.signal("ws://mock".to_string()).unwrap();
        let reconnect_errors = Rc::new(Cell::new(0));
        let socket = WebSocket::lazy(scope, url, test_handler(scope))
            .on_error({
                let reconnect_errors = reconnect_errors.clone();
                move |error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::JsError(_))
                    ));
                    reconnect_errors.set(reconnect_errors.get() + 1);
                }
            })
            .build()
            .expect("websocket setup");
        set_url.set("mock://failure".to_string()).unwrap();
        assert!(matches!(
            socket.reconnect(),
            Err(NetError::Recoverable(NetErrorKind::JsError(_)))
        ));
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Error
        );
        assert_eq!(reconnect_errors.get(), 1);
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 0);
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_retry_stops_after_max_elapsed() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .reconnect_policy(
                RetryPolicy::new(3, std::time::Duration::from_millis(5))
                    .max_elapsed(std::time::Duration::from_millis(1))
                    .no_jitter(),
            )
            .build()
            .expect("websocket setup");
        socket.reconnect().expect("websocket reconnect");
        mock_call0("__silex_test_socket", "emitClose");
        TimeoutFuture::new(10).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 1);
        assert_eq!(
            socket.state().get().unwrap(),
            silex_net::ConnectionState::Closed
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_owner_dispose_removes_active_host_registration() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let opened = Rc::new(Cell::new(0));
    let opened_for_scope = opened.clone();
    let scope = root.access();
    let opened_for_assert = opened.clone();
    async move {
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .on_open(move || opened_for_scope.set(opened_for_scope.get() + 1))
            .build()
            .expect("websocket setup");
        socket.reconnect().expect("websocket reconnect");
        mock_call0("__silex_test_socket", "emitOpen");
        TimeoutFuture::new(0).await;
        assert_eq!(opened_for_assert.get(), 1);
    }
    .await;

    root.close().expect("root cleanup");
    assert!(mock_property_is_cleared("__silex_test_socket", "onopen"));
    assert!(mock_property_is_cleared("__silex_test_socket", "onmessage"));
    assert!(mock_property_is_cleared("__silex_test_socket", "onerror"));
    assert!(mock_property_is_cleared("__silex_test_socket", "onclose"));
    mock_call0("__silex_test_socket", "emitOpen");
    mock_call1("__silex_test_socket", "emitMessage", "late");
    assert_eq!(opened.get(), 1);
    assert!(
        mock_property("__silex_test_socket", "closeCalls")
            .as_f64()
            .unwrap()
            >= 1.0
    );
}

#[wasm_bindgen_test(async)]
async fn event_stream_host_bridge_covers_named_messages_reconnect_and_cleanup() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let opened = Rc::new(Cell::new(0));
        let errors = Rc::new(Cell::new(0));
        let stream = EventStream::lazy(scope, "http://mock", test_handler(scope))
            .event("update")
            .max_messages(2)
            .on_open({
                let opened = opened.clone();
                move || opened.set(opened.get() + 1)
            })
            .on_error({
                let errors = errors.clone();
                move |error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::TransportUnavailable)
                    ));
                    errors.set(errors.get() + 1);
                }
            })
            .build()
            .expect("event stream setup");

        stream.reconnect().expect("event stream reconnect");
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            1
        );
        mock_call0("__silex_test_event_source", "emitOpen");
        assert_eq!(
            stream.state().get().unwrap(),
            silex_net::ConnectionState::Connected
        );
        assert_eq!(opened.get(), 1);

        mock_call2("__silex_test_event_source", "emitNamed", "update", "1");
        mock_call2("__silex_test_event_source", "emitNamed", "update", "2");
        mock_call2("__silex_test_event_source", "emitNamed", "update", "3");
        let messages = stream.raw_messages().get().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].event.as_deref(), Some("update"));
        assert_eq!(messages[0].data, "2");
        assert_eq!(messages[1].data, "3");

        #[cfg(feature = "json")]
        {
            assert_eq!(
                stream
                    .messages::<u32>()
                    .expect("message signal")
                    .get()
                    .unwrap(),
                vec![2, 3]
            );
            assert_eq!(
                stream
                    .last_message::<u32>()
                    .expect("last message signal")
                    .get()
                    .unwrap(),
                Some(3)
            );
            assert_eq!(
                stream
                    .latest_messages::<u32>(2)
                    .expect("latest message signal")
                    .get()
                    .unwrap(),
                vec![3, 2]
            );
        }

        mock_call0("__silex_test_event_source", "emitError");
        assert_eq!(errors.get(), 1);
        assert_eq!(
            stream.error().get().unwrap(),
            Some(NetError::Recoverable(NetErrorKind::TransportUnavailable))
        );
        assert_eq!(
            stream.state().get().unwrap(),
            silex_net::ConnectionState::Error
        );
        stream.reconnect().expect("explicit event stream reconnect");
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            2
        );
        mock_call0("__silex_test_event_source", "emitOpen");
        assert_eq!(opened.get(), 2);
        assert_eq!(stream.error().get().unwrap(), None);

        stream.close().expect("event stream close");
        assert_eq!(
            stream.state().get().unwrap(),
            silex_net::ConnectionState::Closed
        );
        assert!(mock_property_is_cleared(
            "__silex_test_event_source",
            "onopen"
        ));
        assert!(mock_property_is_cleared(
            "__silex_test_event_source",
            "onmessage"
        ));
        assert!(mock_property_is_cleared(
            "__silex_test_event_source",
            "onerror"
        ));
        assert!(
            mock_property("__silex_test_event_source", "removeCalls")
                .as_f64()
                .unwrap()
                >= 1.0
        );
        mock_call2("__silex_test_event_source", "emitNamed", "update", "late");
        assert_eq!(stream.raw_messages().get().unwrap().len(), 2);
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn event_stream_callbacks_can_control_connection_after_state_restore() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let connection_slot: Rc<Cell<Option<EventStreamConnection<'_>>>> = Rc::new(Cell::new(None));
        let opened = Rc::new(Cell::new(0));
        let connection_for_open = connection_slot.clone();
        let opened_for_callback = opened.clone();
        let stream = EventStream::lazy(scope, "http://mock", test_handler(scope))
            .on_open(move || {
                opened_for_callback.set(opened_for_callback.get() + 1);
                let stream = connection_for_open
                    .get()
                    .expect("connection must be available in on_open");
                stream.close().expect("event stream close from callback");
                stream
                    .reconnect()
                    .expect("event stream reconnect from callback");
            })
            .build()
            .expect("event stream setup");
        connection_slot.set(Some(stream));

        stream.reconnect().expect("event stream reconnect");
        mock_call0("__silex_test_event_source", "emitOpen");
        TimeoutFuture::new(0).await;
        assert_eq!(opened.get(), 1);
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            2
        );
        assert_eq!(
            stream.state().get().unwrap(),
            silex_net::ConnectionState::Connecting
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn event_stream_constructor_failure_reports_error_before_connection_creation() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let scope = root.access();
    async move {
        let errors = Rc::new(Cell::new(0));
        let result = EventStream::builder(scope, "mock://failure", test_handler(scope))
            .on_error({
                let errors = errors.clone();
                move |error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::JsError(_))
                    ));
                    errors.set(errors.get() + 1);
                }
            })
            .build();
        assert!(matches!(
            result,
            Err(NetError::Recoverable(NetErrorKind::JsError(_)))
        ));
        assert_eq!(errors.get(), 1);
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            0
        );

        let (url, set_url) = scope.signal("http://mock".to_string()).unwrap();
        let reconnect_errors = Rc::new(Cell::new(0));
        let stream = EventStream::lazy(scope, url, test_handler(scope))
            .on_error({
                let reconnect_errors = reconnect_errors.clone();
                move |error| {
                    assert!(matches!(
                        error,
                        NetError::Recoverable(NetErrorKind::JsError(_))
                    ));
                    reconnect_errors.set(reconnect_errors.get() + 1);
                }
            })
            .build()
            .expect("event stream setup");
        set_url.set("mock://failure".to_string()).unwrap();
        assert!(matches!(
            stream.reconnect(),
            Err(NetError::Recoverable(NetErrorKind::JsError(_)))
        ));
        assert_eq!(
            stream.state().get().unwrap(),
            silex_net::ConnectionState::Error
        );
        assert_eq!(reconnect_errors.get(), 1);
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            0
        );
    }
    .await;
    root.close().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn event_stream_owner_dispose_removes_active_host_registration() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime setup");
    let opened = Rc::new(Cell::new(0));
    let opened_for_scope = opened.clone();
    let scope = root.access();
    let opened_for_assert = opened.clone();
    async move {
        let stream = EventStream::lazy(scope, "http://mock", test_handler(scope))
            .event("update")
            .on_open(move || opened_for_scope.set(opened_for_scope.get() + 1))
            .build()
            .expect("event stream setup");
        stream.reconnect().expect("event stream reconnect");
        mock_call0("__silex_test_event_source", "emitOpen");
        TimeoutFuture::new(0).await;
        assert_eq!(opened_for_assert.get(), 1);
    }
    .await;

    root.close().expect("root cleanup");
    assert!(mock_property_is_cleared(
        "__silex_test_event_source",
        "onopen"
    ));
    assert!(mock_property_is_cleared(
        "__silex_test_event_source",
        "onerror"
    ));
    assert!(
        mock_property("__silex_test_event_source", "removeCalls")
            .as_f64()
            .unwrap()
            >= 1.0
    );
    mock_call0("__silex_test_event_source", "emitOpen");
    mock_call2("__silex_test_event_source", "emitNamed", "update", "late");
    assert_eq!(opened.get(), 1);
    assert!(
        mock_property("__silex_test_event_source", "closeCalls")
            .as_f64()
            .unwrap()
            >= 1.0
    );
}
