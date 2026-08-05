#![cfg(target_arch = "wasm32")]

use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use gloo_timers::future::TimeoutFuture;
use silex_core::reactivity::ResourceState;
use silex_core::{Runtime, TaskHandle};
use silex_net::{BrowserTransport, EventStream, HttpMethod, RequestBody, RequestSpec, WebSocket};
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
