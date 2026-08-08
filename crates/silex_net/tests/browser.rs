#![cfg(target_arch = "wasm32")]

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Function, Reflect};
use silex_core::reactivity::{MutationState, ResourceState};
use silex_core::{ErrorReporter, Runtime, Scope, TaskHandle};
use silex_net::{
    BrowserTransport, EventStream, EventStreamConnection, HttpMethod, HttpResponse, NetError,
    RequestBody, RequestSpec, RetryPolicy, Transport, TransportFuture, WebSocket,
    WebSocketConnection,
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope.error_handler(|_| {})
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
                        this.sent.push(data);
                    },
                    close: function () {
                        this.closeCalls += 1;
                        this.readyState = 3;
                        if (this.onclose) {
                            this.onclose(new CloseEvent("close", {
                                code: 1000,
                                reason: "closed"
                            }));
                        }
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
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (source, _) = scope.signal(1_u32);
        let resource =
            silex_net::HttpClient::get(scope, "data:text/plain,hello", test_handler(scope))
                .as_resource(source, None);
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "hello"
        ));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn resource_runs_interceptor_once_and_rejects_custom_status() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (source, _) = scope.signal(1_u32);
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
                .as_resource(source, None);
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "ok"
        ));
        assert_eq!(interceptor_calls.get(), 1);
        assert_eq!(transport_calls.get(), 1);
    })
    .await;
    root.dispose().expect("root cleanup");

    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (source, _) = scope.signal(1_u32);
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
                    assert!(matches!(error, NetError::HttpStatus { status: 503, .. }));
                    error_calls_for_hook.set(error_calls_for_hook.get() + 1);
                })
                .on_retry(move |_, _, _, error| {
                    assert!(matches!(error, NetError::HttpStatus { status: 503, .. }));
                    retry_calls_for_hook.set(retry_calls_for_hook.get() + 1);
                })
                .as_resource(source, None);
        TimeoutFuture::new(1).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Error(NetError::HttpStatus { status: 503, .. })
        ));
        assert_eq!(transport_calls.get(), 3);
        assert_eq!(error_calls.get(), 3);
        assert_eq!(retry_calls.get(), 2);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn resource_replacement_keeps_new_request_result() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (query, set_query) = scope.signal("first".to_string());
        let (source, _) = scope.signal(1_u32);
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
        .as_resource(source, None);
        TimeoutFuture::new(0).await;
        assert_eq!(calls.get(), 1);
        set_query.set("second".to_string());
        TimeoutFuture::new(30).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "second"
        ));
        assert_eq!(calls.get(), 2);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn mutation_preflight_error_does_not_enter_pending() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.run();
    let target_root = target_runtime.run();
    source_root.with_scope(|source_scope| {
        let (foreign, _) = source_scope.signal("foreign".to_string());
        let foreign_inputs = silex_core::runtime_inputs_of(foreign);
        target_root.with_scope(|target_scope| {
            let mutation = silex_net::HttpClient::post(
                target_scope,
                "https://example.test/mutate",
                test_handler(target_scope),
            )
            .as_mutation_with(move |_| {
                let inputs = foreign_inputs.clone();
                let body = silex_net::ValueResolver::dynamic_with_inputs(
                    || "foreign".to_string(),
                    || "foreign".to_string(),
                    inputs,
                );
                silex_net::HttpClient::post(
                    target_scope,
                    "https://example.test/mutate",
                    test_handler(target_scope),
                )
                .text_body(body)
            });
            mutation.mutate(());
            assert!(matches!(
                mutation.state.get(),
                MutationState::Error(NetError::InvalidConfiguration(_))
            ));
        });
    });
    source_root.dispose().expect("source root cleanup");
    target_root.dispose().expect("target root cleanup");
}

#[wasm_bindgen_test(async)]
async fn mutation_commits_only_the_latest_completion() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let calls = Rc::new(Cell::new(0));
        let mutation =
            silex_net::HttpClient::get(scope, "https://example.test/mutation", test_handler(scope))
                .as_mutation_with({
                    let calls = calls.clone();
                    move |id: u32| {
                        silex_net::HttpClient::get(
                            scope,
                            "https://example.test/mutation",
                            test_handler(scope),
                        )
                        .query("id", id)
                        .transport(MutationTransport {
                            calls: calls.clone(),
                        })
                    }
                });
        mutation.mutate(1);
        mutation.mutate(2);
        assert!(matches!(mutation.state.get(), MutationState::Pending));
        TimeoutFuture::new(30).await;
        assert!(matches!(
            mutation.state.get(),
            MutationState::Success(value) if value == "two"
        ));
        assert_eq!(calls.get(), 2);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn cache_first_does_not_treat_default_as_history() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (source, _) = scope.signal(1_u32);
        let resource =
            silex_net::HttpClient::get(scope, "data:text/plain,cache", test_handler(scope))
                .cache_with_default(silex_net::CachePolicy::CacheFirst, "default".to_string())
                .as_resource(source, None);
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "cache"
        ));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[cfg(feature = "persist")]
#[wasm_bindgen_test(async)]
async fn cache_first_reloads_history_when_request_key_changes() {
    let url = "https://example.test/cache-key";
    let first_spec = RequestSpec {
        method: HttpMethod::Get,
        url: format!("{url}?value=first"),
        headers: Vec::new(),
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
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (query, set_query) = scope.signal("first".to_string());
        let (source, _) = scope.signal(1_u32);
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .query("value", query)
            .cache_with_default(silex_net::CachePolicy::CacheFirst, "default".to_string())
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 200,
                body: "network",
                delay_ms: 0,
            })
            .as_resource(source, None);
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 0);

        set_query.set("second".to_string());
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "network"
        ));
        assert_eq!(calls.get(), 1);

        set_query.set("first".to_string());
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 1);
    })
    .await;
    root.dispose().expect("root cleanup");
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
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (source, set_source) = scope.signal(1_u32);
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .cache_with_default(
                silex_net::CachePolicy::StaleWhileRevalidate,
                "default".to_string(),
            )
            .transport(GenerationTransport {
                calls: calls.clone(),
            })
            .as_resource(source, None);
        TimeoutFuture::new(10).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 1);

        set_source.set(2);
        TimeoutFuture::new(30).await;
        let state = resource.state.get();
        assert_eq!(
            state,
            ResourceState::Ready("fresh".to_string()),
            "state after SWR generation test, transport calls={}",
            calls.get()
        );
        assert_eq!(calls.get(), 2);
    })
    .await;
    root.dispose().expect("root cleanup");
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
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let (source, _) = scope.signal(1_u32);
        let calls = Rc::new(Cell::new(0));
        let resource = silex_net::HttpClient::get(scope, url, test_handler(scope))
            .cache_with_default(silex_net::CachePolicy::NetworkFirst, "default".to_string())
            .transport(ScriptedTransport {
                calls: calls.clone(),
                status: 503,
                body: "unavailable",
                delay_ms: 0,
            })
            .as_resource(source, None);
        TimeoutFuture::new(0).await;
        assert!(matches!(
            resource.state.get(),
            ResourceState::Ready(value) if value == "history"
        ));
        assert_eq!(calls.get(), 1);
    })
    .await;
    root.dispose().expect("root cleanup");
    storage
        .remove_item(&storage_key)
        .expect("remove cache history");
}

#[wasm_bindgen_test(async)]
async fn task_cancel_drops_pending_scoped_future() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let dropped_for_scope = dropped.clone();
    root.with_scope(|scope| async move {
        let task: TaskHandle = scope.spawn_scoped(
            PendingFuture {
                dropped: dropped_for_scope.clone(),
            },
            test_handler(scope),
        );
        TimeoutFuture::new(10).await;
        assert_eq!(dropped_for_scope.get(), 0);
        task.cancel();
        TimeoutFuture::new(0).await;
        assert_eq!(dropped_for_scope.get(), 1);
    })
    .await;
    root.dispose().expect("root cleanup");
    assert_eq!(dropped.get(), 1);
}

#[wasm_bindgen_test]
fn lazy_connections_validate_scope_without_opening_host_resources() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let socket = WebSocket::lazy(scope, "wss://example.test", test_handler(scope)).build();
        let stream =
            EventStream::lazy(scope, "https://example.test/events", test_handler(scope)).build();
        assert!(socket.state().get().is_active() == false);
        assert!(stream.state().get().is_active() == false);
    });
}

#[wasm_bindgen_test(async)]
async fn websocket_host_bridge_covers_events_retry_and_manual_close() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
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
                        NetError::JsError(message) if message == "broken"
                    ));
                    errors.set(errors.get() + 1);
                }
            })
            .on_close({
                let closed = closed.clone();
                move |_, _| closed.set(closed.get() + 1)
            })
            .build();

        socket.reconnect();
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 1);
        mock_call0("__silex_test_socket", "emitOpen");
        TimeoutFuture::new(0).await;
        assert_eq!(socket.state().get(), silex_net::ConnectionState::Connected);
        assert_eq!(opened.get(), 1);

        mock_call1("__silex_test_socket", "emitMessage", "first");
        assert_eq!(socket.raw_message().get().as_deref(), Some("first"));
        socket.send_text("outbound").expect("send on mock socket");
        let sent = mock_property("__silex_test_socket", "sent")
            .dyn_into::<Array>()
            .expect("sent array");
        assert_eq!(sent.length(), 1);
        assert_eq!(sent.get(0).as_string().as_deref(), Some("outbound"));

        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(closed.get(), 1);
        assert_eq!(socket.state().get(), silex_net::ConnectionState::Closed);
        TimeoutFuture::new(0).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 2);
        mock_call0("__silex_test_socket", "emitOpen");
        assert_eq!(opened.get(), 2);

        mock_call1("__silex_test_socket", "emitError", "broken");
        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(errors.get(), 1);
        assert!(matches!(
            socket.error().get(),
            Some(NetError::JsError(message)) if message == "broken"
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
        assert_eq!(socket.error().get(), None);

        socket.close().expect("manual close");
        assert_eq!(socket.state().get(), silex_net::ConnectionState::Closed);
        assert!(mock_property_is_cleared("__silex_test_socket", "onopen"));
        assert!(mock_property_is_cleared("__silex_test_socket", "onmessage"));
        assert!(mock_property_is_cleared("__silex_test_socket", "onerror"));
        assert!(mock_property_is_cleared("__silex_test_socket", "onclose"));
        mock_call1("__silex_test_socket", "emitMessage", "late");
        assert_eq!(socket.raw_message().get().as_deref(), Some("first"));
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_retry_window_counts_continuous_pre_open_failures() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .reconnect_policy(RetryPolicy::new(3, std::time::Duration::ZERO).no_jitter())
            .build();

        socket.reconnect();
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
            3,
            "continuous pre-open failures must exhaust one retry window"
        );

        socket.reconnect();
        assert_eq!(
            mock_instance_count("__silex_test_socket_instances"),
            4,
            "manual reconnect must start a fresh retry window"
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_callbacks_can_control_connection_after_state_restore() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
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
                    .reconnect();
            })
            .build();
        connection_slot.set(Some(socket));

        socket.reconnect();
        mock_call0("__silex_test_socket", "emitOpen");
        TimeoutFuture::new(0).await;
        assert!(send_succeeded.get());

        mock_call0("__silex_test_socket", "emitClose");
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 2);
        assert_eq!(socket.state().get(), silex_net::ConnectionState::Connecting);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_constructor_failure_reports_error_before_connection_creation() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let errors = Rc::new(Cell::new(0));
        let result = WebSocket::connect(scope, "mock://failure", test_handler(scope))
            .on_error({
                let errors = errors.clone();
                move |error| {
                    assert!(matches!(error, NetError::JsError(_)));
                    errors.set(errors.get() + 1);
                }
            })
            .try_build();
        assert!(matches!(result, Err(NetError::JsError(_))));
        assert_eq!(errors.get(), 1);
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 0);

        let (url, set_url) = scope.signal("ws://mock".to_string());
        let reconnect_errors = Rc::new(Cell::new(0));
        let socket = WebSocket::lazy(scope, url, test_handler(scope))
            .on_error({
                let reconnect_errors = reconnect_errors.clone();
                move |error| {
                    assert!(matches!(error, NetError::JsError(_)));
                    reconnect_errors.set(reconnect_errors.get() + 1);
                }
            })
            .build();
        set_url.set("mock://failure".to_string());
        assert!(matches!(socket.try_reconnect(), Err(NetError::JsError(_))));
        assert_eq!(socket.state().get(), silex_net::ConnectionState::Error);
        assert_eq!(reconnect_errors.get(), 1);
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 0);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_retry_stops_after_max_elapsed() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
            .reconnect_policy(
                RetryPolicy::new(3, std::time::Duration::from_millis(5))
                    .max_elapsed(std::time::Duration::from_millis(1))
                    .no_jitter(),
            )
            .build();
        socket.reconnect();
        mock_call0("__silex_test_socket", "emitClose");
        TimeoutFuture::new(10).await;
        assert_eq!(mock_instance_count("__silex_test_socket_instances"), 1);
        assert_eq!(socket.state().get(), silex_net::ConnectionState::Closed);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn websocket_owner_dispose_removes_active_host_registration() {
    let _host = MockHost::websocket();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let opened = Rc::new(Cell::new(0));
    let opened_for_scope = opened.clone();
    root.with_scope(|scope| {
        let opened_for_assert = opened.clone();
        async move {
            let socket = WebSocket::lazy(scope, "ws://mock", test_handler(scope))
                .on_open(move || opened_for_scope.set(opened_for_scope.get() + 1))
                .build();
            socket.reconnect();
            mock_call0("__silex_test_socket", "emitOpen");
            TimeoutFuture::new(0).await;
            assert_eq!(opened_for_assert.get(), 1);
        }
    })
    .await;

    root.dispose().expect("root cleanup");
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
    let root = runtime.run();
    root.with_scope(|scope| async move {
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
                    assert!(matches!(error, NetError::TransportUnavailable));
                    errors.set(errors.get() + 1);
                }
            })
            .build();

        stream.reconnect();
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            1
        );
        mock_call0("__silex_test_event_source", "emitOpen");
        assert_eq!(stream.state().get(), silex_net::ConnectionState::Connected);
        assert_eq!(opened.get(), 1);

        mock_call2("__silex_test_event_source", "emitNamed", "update", "1");
        mock_call2("__silex_test_event_source", "emitNamed", "update", "2");
        mock_call2("__silex_test_event_source", "emitNamed", "update", "3");
        let messages = stream.raw_messages().get();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].event.as_deref(), Some("update"));
        assert_eq!(messages[0].data, "2");
        assert_eq!(messages[1].data, "3");

        #[cfg(feature = "json")]
        {
            assert_eq!(stream.messages::<u32>().get(), vec![2, 3]);
            assert_eq!(stream.last_message::<u32>().get(), Some(3));
            assert_eq!(stream.latest_messages::<u32>(2).get(), vec![3, 2]);
        }

        mock_call0("__silex_test_event_source", "emitError");
        assert_eq!(errors.get(), 1);
        assert_eq!(stream.error().get(), Some(NetError::TransportUnavailable));
        assert_eq!(stream.state().get(), silex_net::ConnectionState::Error);
        stream
            .try_reconnect()
            .expect("explicit event stream reconnect");
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            2
        );
        mock_call0("__silex_test_event_source", "emitOpen");
        assert_eq!(opened.get(), 2);
        assert_eq!(stream.error().get(), None);

        stream.close();
        assert_eq!(stream.state().get(), silex_net::ConnectionState::Closed);
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
        assert_eq!(stream.raw_messages().get().len(), 2);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn event_stream_callbacks_can_control_connection_after_state_restore() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
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
                stream.close();
                stream.reconnect();
            })
            .build();
        connection_slot.set(Some(stream));

        stream.reconnect();
        mock_call0("__silex_test_event_source", "emitOpen");
        TimeoutFuture::new(0).await;
        assert_eq!(opened.get(), 1);
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            2
        );
        assert_eq!(stream.state().get(), silex_net::ConnectionState::Connecting);
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn event_stream_constructor_failure_reports_error_before_connection_creation() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| async move {
        let errors = Rc::new(Cell::new(0));
        let result = EventStream::builder(scope, "mock://failure", test_handler(scope))
            .on_error({
                let errors = errors.clone();
                move |error| {
                    assert!(matches!(error, NetError::JsError(_)));
                    errors.set(errors.get() + 1);
                }
            })
            .try_build();
        assert!(matches!(result, Err(NetError::JsError(_))));
        assert_eq!(errors.get(), 1);
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            0
        );

        let (url, set_url) = scope.signal("http://mock".to_string());
        let reconnect_errors = Rc::new(Cell::new(0));
        let stream = EventStream::lazy(scope, url, test_handler(scope))
            .on_error({
                let reconnect_errors = reconnect_errors.clone();
                move |error| {
                    assert!(matches!(error, NetError::JsError(_)));
                    reconnect_errors.set(reconnect_errors.get() + 1);
                }
            })
            .build();
        set_url.set("mock://failure".to_string());
        assert!(matches!(stream.try_reconnect(), Err(NetError::JsError(_))));
        assert_eq!(stream.state().get(), silex_net::ConnectionState::Error);
        assert_eq!(reconnect_errors.get(), 1);
        assert_eq!(
            mock_instance_count("__silex_test_event_source_instances"),
            0
        );
    })
    .await;
    root.dispose().expect("root cleanup");
}

#[wasm_bindgen_test(async)]
async fn event_stream_owner_dispose_removes_active_host_registration() {
    let _host = MockHost::event_source();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let opened = Rc::new(Cell::new(0));
    let opened_for_scope = opened.clone();
    root.with_scope(|scope| {
        let opened_for_assert = opened.clone();
        async move {
            let stream = EventStream::lazy(scope, "http://mock", test_handler(scope))
                .event("update")
                .on_open(move || opened_for_scope.set(opened_for_scope.get() + 1))
                .build();
            stream.reconnect();
            mock_call0("__silex_test_event_source", "emitOpen");
            TimeoutFuture::new(0).await;
            assert_eq!(opened_for_assert.get(), 1);
        }
    })
    .await;

    root.dispose().expect("root cleanup");
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
