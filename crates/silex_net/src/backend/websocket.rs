use std::{cell::Cell, rc::Rc, time::Duration};

use gloo_timers::future::sleep;
use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, MessageEvent, WebSocket as JsWebSocket};

use silex_core::{
    CompletionToken, Memo, ReadSignal, RuntimeInputs, Scope, StoredValue, TaskHandle, WriteSignal,
};

use crate::{
    NetError,
    builder::{IntoNetValue, ValueResolver},
    state::{ConnectionState, RetryPolicy},
};

pub struct WebSocket;

impl WebSocket {
    pub fn connect<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
    ) -> WebSocketBuilder<'scope> {
        WebSocketBuilder::new(scope, url.into_net_value())
    }

    pub fn open<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
    ) -> Result<WebSocketConnection<'scope>, NetError> {
        Self::connect(scope, url).try_build()
    }

    pub fn lazy<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
    ) -> WebSocketBuilder<'scope> {
        Self::connect(scope, url).auto_connect(false)
    }
}

#[derive(Copy, Clone)]
pub struct WebSocketConnection<'scope> {
    scope: Scope<'scope>,
    inner: StoredValue<'scope, WebSocketInner<'scope>>,
    state: ReadSignal<'scope, ConnectionState>,
    message: ReadSignal<'scope, Option<String>>,
    error: ReadSignal<'scope, Option<String>>,
}

#[derive(Clone)]
enum WebSocketEvent {
    Open {
        generation: u64,
    },
    Message {
        generation: u64,
        data: String,
    },
    Error {
        generation: u64,
        message: String,
    },
    Close {
        generation: u64,
        code: u16,
        reason: String,
    },
    Retry {
        generation: u64,
    },
}

struct HostRegistration {
    socket: JsWebSocket,
    gate: Rc<Cell<bool>>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    _on_close: Closure<dyn FnMut(web_sys::CloseEvent)>,
}

impl HostRegistration {
    fn new(socket: JsWebSocket, generation: u64, token: &CompletionToken<WebSocketEvent>) -> Self {
        let gate = Rc::new(Cell::new(true));

        let open_gate = gate.clone();
        let open_token = token.clone();
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            if open_gate.get() {
                let _ = open_token.submit(WebSocketEvent::Open { generation });
            }
        }) as Box<dyn FnMut(Event)>);

        let message_gate = gate.clone();
        let message_token = token.clone();
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            if message_gate.get() {
                let data = event.data().as_string().unwrap_or_default();
                let _ = message_token.submit(WebSocketEvent::Message { generation, data });
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        let error_gate = gate.clone();
        let error_token = token.clone();
        let on_error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
            if error_gate.get() {
                let _ = error_token.submit(WebSocketEvent::Error {
                    generation,
                    message: event.message(),
                });
            }
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

        let close_gate = gate.clone();
        let close_token = token.clone();
        let on_close = Closure::wrap(Box::new(move |event: web_sys::CloseEvent| {
            if close_gate.get() {
                let _ = close_token.submit(WebSocketEvent::Close {
                    generation,
                    code: event.code(),
                    reason: event.reason(),
                });
            }
        }) as Box<dyn FnMut(web_sys::CloseEvent)>);

        socket.set_onopen(Some(on_open.as_ref().unchecked_ref::<Function>()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref::<Function>()));
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref::<Function>()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref::<Function>()));

        Self {
            socket,
            gate,
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
            _on_close: on_close,
        }
    }
}

impl Drop for HostRegistration {
    fn drop(&mut self) {
        self.gate.set(false);
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}

struct WebSocketInner<'scope> {
    url: ValueResolver<'scope>,
    protocols: Vec<String>,
    retry: Option<RetryPolicy>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(String) + 'scope>>,
    on_close: Vec<Rc<dyn Fn(u16, String) + 'scope>>,
    set_state: WriteSignal<'scope, ConnectionState>,
    set_message: WriteSignal<'scope, Option<String>>,
    set_error: WriteSignal<'scope, Option<String>>,
    completion: CompletionToken<WebSocketEvent>,
    scope: Scope<'scope>,
    registration: Option<HostRegistration>,
    generation: u64,
    retry_generation: Option<u64>,
    retry_task: Option<TaskHandle>,
    retry_attempt: u32,
    retry_started_at: Option<f64>,
}

impl Drop for WebSocketInner<'_> {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl<'scope> WebSocketInner<'scope> {
    fn cleanup(&mut self) {
        if let Some(task) = self.retry_task.take() {
            task.cancel();
        }
        self.retry_generation = None;
        self.registration.take();
        self.completion.cancel();
    }

    fn cancel_retry(&mut self) {
        if let Some(task) = self.retry_task.take() {
            task.cancel();
        }
        self.retry_generation = None;
    }

    fn socket(&self) -> Result<JsWebSocket, NetError> {
        let url = self.url.resolve();
        if self.protocols.is_empty() {
            JsWebSocket::new(&url).map_err(NetError::from)
        } else {
            let protocols = js_sys::Array::new();
            for protocol in &self.protocols {
                protocols.push(&wasm_bindgen::JsValue::from_str(protocol));
            }
            JsWebSocket::new_with_str_sequence(&url, &protocols.into()).map_err(NetError::from)
        }
    }

    fn try_open_current(&mut self) -> Result<(), NetError> {
        self.cancel_retry();
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.retry_attempt = 0;
        self.retry_started_at = None;
        self.set_state.set(ConnectionState::Connecting);

        let socket = match self.socket() {
            Ok(socket) => socket,
            Err(error) => {
                self.set_error.set(Some(format!("{error:?}")));
                self.set_state.set(ConnectionState::Error);
                for handler in &self.on_error {
                    handler(format!("{error:?}"));
                }
                return Err(error);
            }
        };
        self.registration = Some(HostRegistration::new(socket, generation, &self.completion));
        Ok(())
    }

    fn schedule_retry(&mut self, generation: u64) {
        if self.retry_generation == Some(generation) {
            return;
        }
        let Some(policy) = self.retry else {
            return;
        };
        let max_attempts = policy.max_attempts.max(1);
        self.retry_attempt = self.retry_attempt.saturating_add(1);
        if self.retry_attempt >= max_attempts {
            return;
        }

        let now = js_sys::Date::now();
        let started_at = *self.retry_started_at.get_or_insert(now);
        if let Some(max_elapsed) = policy.max_elapsed {
            let elapsed = Duration::from_millis((now - started_at).max(0.0) as u64);
            if elapsed >= max_elapsed {
                return;
            }
        }

        let delay = policy.delay_for_attempt(self.retry_attempt);
        let token = self.completion.clone();
        self.retry_generation = Some(generation);
        self.retry_task = Some(self.scope.spawn_scoped(async move {
            if delay > Duration::from_millis(0) {
                sleep(delay).await;
            }
            let _ = token.submit(WebSocketEvent::Retry { generation });
        }));
    }

    fn handle_event(&mut self, event: WebSocketEvent) {
        match event {
            WebSocketEvent::Open { generation } if generation == self.generation => {
                self.retry_task.take();
                self.retry_generation = None;
                self.retry_attempt = 0;
                self.retry_started_at = None;
                self.set_state.set(ConnectionState::Connected);
                for handler in &self.on_open {
                    handler();
                }
            }
            WebSocketEvent::Message { generation, data } if generation == self.generation => {
                self.set_message.set(Some(data));
                self.set_state.set(ConnectionState::Connected);
            }
            WebSocketEvent::Error {
                generation,
                message,
            } if generation == self.generation => {
                self.set_error.set(Some(message.clone()));
                self.set_state.set(ConnectionState::Error);
                for handler in &self.on_error {
                    handler(message.clone());
                }
                self.schedule_retry(generation);
            }
            WebSocketEvent::Close {
                generation,
                code,
                reason,
            } if generation == self.generation => {
                self.registration.take();
                self.set_state.set(ConnectionState::Closed);
                for handler in &self.on_close {
                    handler(code, reason.clone());
                }
                self.schedule_retry(generation);
            }
            WebSocketEvent::Retry { generation } if generation == self.generation => {
                self.retry_task.take();
                self.retry_generation = None;
                let _ = self.try_open_current();
            }
            _ => {}
        }
    }

    fn close(&mut self) {
        self.cancel_retry();
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        self.set_state.set(ConnectionState::Closed);
    }
}

impl<'scope> WebSocketConnection<'scope> {
    pub fn is_connected(&self) -> Memo<'scope, bool> {
        let state = self.state;
        self.scope.memo(move |_| state.get().is_connected())
    }

    pub fn is_connecting(&self) -> Memo<'scope, bool> {
        let state = self.state;
        self.scope
            .memo(move |_| matches!(state.get(), ConnectionState::Connecting))
    }

    pub fn is_closed(&self) -> Memo<'scope, bool> {
        let state = self.state;
        self.scope.memo(move |_| {
            matches!(
                state.get(),
                ConnectionState::Closed | ConnectionState::Disconnected
            )
        })
    }

    pub fn state(&self) -> ReadSignal<'scope, ConnectionState> {
        self.state
    }

    #[cfg(feature = "json")]
    pub fn message<T>(&self) -> Memo<'scope, Option<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let message = self.message;
        self.scope.memo(move |_| {
            message
                .get()
                .and_then(|raw| serde_json::from_str(&raw).ok())
        })
    }

    pub fn state_str(&self) -> Memo<'scope, &'static str> {
        let state = self.state;
        self.scope.memo(move |_| state.get().as_str())
    }

    pub fn raw_message(&self) -> ReadSignal<'scope, Option<String>> {
        self.message
    }

    pub fn error(&self) -> ReadSignal<'scope, Option<String>> {
        self.error
    }

    pub fn send(&self, value: impl Into<String>) -> Result<(), NetError> {
        let message = value.into();
        self.inner.with(|inner| {
            if let Some(registration) = &inner.registration {
                registration
                    .socket
                    .send_with_str(&message)
                    .map_err(NetError::from)
            } else {
                Err(NetError::ConnectionClosed(
                    "WebSocket is not connected".to_string(),
                ))
            }
        })
    }

    pub fn send_text(&self, value: impl Into<String>) -> Result<(), NetError> {
        self.send(value)
    }

    #[cfg(feature = "json")]
    pub fn send_json<T>(&self, value: &T) -> Result<(), NetError>
    where
        T: serde::Serialize,
    {
        let payload = serde_json::to_string(value)
            .map_err(|error| NetError::SerializeError(error.to_string()))?;
        self.send(payload)
    }

    pub fn close(&self) -> Result<(), NetError> {
        self.inner.update(WebSocketInner::close);
        Ok(())
    }

    pub fn try_reconnect(&self) -> Result<(), NetError> {
        if !matches!(
            self.state.get(),
            ConnectionState::Closed | ConnectionState::Disconnected | ConnectionState::Error
        ) {
            return Ok(());
        }
        self.inner.update(WebSocketInner::try_open_current)
    }

    pub fn reconnect(&self) {
        let _ = self.try_reconnect();
    }

    pub fn toggle(&self) {
        if self.state.get().is_active() {
            let _ = self.close();
        } else {
            self.reconnect();
        }
    }
}

#[derive(Clone)]
pub struct WebSocketBuilder<'scope> {
    scope: Scope<'scope>,
    url: ValueResolver<'scope>,
    protocols: Vec<String>,
    auto_connect: bool,
    reconnect: Option<RetryPolicy>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(String) + 'scope>>,
    on_close: Vec<Rc<dyn Fn(u16, String) + 'scope>>,
}

impl<'scope> WebSocketBuilder<'scope> {
    fn new(scope: Scope<'scope>, url: ValueResolver<'scope>) -> Self {
        Self {
            scope,
            url,
            protocols: Vec::new(),
            auto_connect: true,
            reconnect: None,
            on_open: Vec::new(),
            on_error: Vec::new(),
            on_close: Vec::new(),
        }
    }

    pub fn auto_connect(mut self, auto_connect: bool) -> Self {
        self.auto_connect = auto_connect;
        self
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocols.push(protocol.into());
        self
    }

    pub fn reconnect(mut self, attempts: u32, delay: Duration) -> Self {
        self.reconnect = Some(RetryPolicy::new(attempts, delay));
        self
    }

    pub fn on_open(mut self, handler: impl Fn() + 'scope) -> Self {
        self.on_open.push(Rc::new(handler));
        self
    }

    pub fn on_error(mut self, handler: impl Fn(String) + 'scope) -> Self {
        self.on_error.push(Rc::new(handler));
        self
    }

    pub fn on_close(mut self, handler: impl Fn(u16, String) + 'scope) -> Self {
        self.on_close.push(Rc::new(handler));
        self
    }

    pub fn try_build(self) -> Result<WebSocketConnection<'scope>, NetError> {
        let Self {
            scope,
            url,
            protocols,
            auto_connect,
            reconnect,
            on_open,
            on_error,
            on_close,
        } = self;
        let mut inputs = RuntimeInputs::new();
        inputs.extend(&url.inputs());
        scope
            .try_validate_inputs(&inputs)
            .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

        let (state, set_state) = scope.signal(if auto_connect {
            ConnectionState::Connecting
        } else {
            ConnectionState::Disconnected
        });
        let (message, set_message) = scope.signal(None::<String>);
        let (error, set_error) = scope.signal(None::<String>);
        let inner_slot = Rc::new(Cell::new(
            None::<StoredValue<'scope, WebSocketInner<'scope>>>,
        ));
        let inner_slot_for_completion = inner_slot.clone();
        let completion = scope.completion(move |event: WebSocketEvent| {
            if let Some(inner) = inner_slot_for_completion.get() {
                inner.update(|inner| inner.handle_event(event));
            }
        });
        let inner = scope.stored(WebSocketInner {
            url,
            protocols,
            retry: reconnect,
            on_open,
            on_error,
            on_close,
            set_state,
            set_message,
            set_error,
            completion,
            scope,
            registration: None,
            generation: 0,
            retry_generation: None,
            retry_task: None,
            retry_attempt: 0,
            retry_started_at: None,
        });
        inner_slot.set(Some(inner));
        scope.on_cleanup(move || {
            if inner.is_alive() {
                inner.update(WebSocketInner::cleanup);
            }
        });

        let connection = WebSocketConnection {
            scope,
            inner,
            state,
            message,
            error,
        };
        if auto_connect && let Err(error) = inner.update(WebSocketInner::try_open_current) {
            inner.update(WebSocketInner::cleanup);
            return Err(error);
        }
        Ok(connection)
    }

    pub fn build(self) -> WebSocketConnection<'scope> {
        self.try_build()
            .unwrap_or_else(|error| panic!("创建 WebSocket 失败: {error:?}"))
    }
}
