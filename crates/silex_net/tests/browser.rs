#![cfg(target_arch = "wasm32")]

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use gloo_timers::future::TimeoutFuture;
use silex_core::reactivity::{MutationState, ResourceState};
use silex_core::{Runtime, TaskHandle};
use silex_net::{
    BrowserTransport, EventStream, HttpMethod, HttpResponse, NetError, RequestBody, RequestSpec,
    RetryPolicy, Transport, TransportFuture, WebSocket,
};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

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
            silex_net::HttpClient::get(scope, "data:text/plain,hello").as_resource(source, None);
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
        let resource = silex_net::HttpClient::get(scope, "https://example.test/success")
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
        let resource = silex_net::HttpClient::get(scope, "https://example.test/status")
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
        let resource = silex_net::HttpClient::get(scope, "https://example.test/replacement")
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
            let mutation = silex_net::HttpClient::post(target_scope, "https://example.test/mutate")
                .as_mutation_with(move |_| {
                    let inputs = foreign_inputs.clone();
                    let body = silex_net::ValueResolver::dynamic_with_inputs(
                        || "foreign".to_string(),
                        || "foreign".to_string(),
                        inputs,
                    );
                    silex_net::HttpClient::post(target_scope, "https://example.test/mutate")
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
        let mutation = silex_net::HttpClient::get(scope, "https://example.test/mutation")
            .as_mutation_with({
                let calls = calls.clone();
                move |id: u32| {
                    silex_net::HttpClient::get(scope, "https://example.test/mutation")
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
        let resource = silex_net::HttpClient::get(scope, "data:text/plain,cache")
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
        let resource = silex_net::HttpClient::get(scope, url)
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
        let resource = silex_net::HttpClient::get(scope, url)
            .cache_with_default(
                silex_net::CachePolicy::StaleWhileRevalidate,
                "default".to_string(),
            )
            .transport(GenerationTransport {
                calls: calls.clone(),
            })
            .as_resource(source, None);
        TimeoutFuture::new(1).await;
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
        let resource = silex_net::HttpClient::get(scope, url)
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
        let task: TaskHandle = scope.spawn_scoped(PendingFuture {
            dropped: dropped_for_scope.clone(),
        });
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
        let socket = WebSocket::lazy(scope, "wss://example.test").build();
        let stream = EventStream::lazy(scope, "https://example.test/events").build();
        assert!(socket.state().get().is_active() == false);
        assert!(stream.state().get().is_active() == false);
    });
}
