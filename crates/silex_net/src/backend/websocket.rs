use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
    time::Duration,
};

use gloo_timers::future::sleep;
use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, MessageEvent, WebSocket as JsWebSocket};

use silex_core::{
    CallbackInvokeError, CompletionSender, ErrorReporter, ReadSignal, Rx, Scope, SilexError,
    SilexResult, StoredValue, TaskHandle, WriteSignal, unwind_safe,
};

use crate::{
    NetError, NetErrorKind,
    builder::{IntoNetValue, ValueResolver},
    state::{ConnectionState, RetryPolicy},
};

pub struct WebSocket;

impl WebSocket {
    pub fn connect<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> WebSocketBuilder<'scope> {
        WebSocketBuilder::new(scope, url.into_net_value(), error_handler)
    }

    pub fn open<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<WebSocketConnection<'scope>, NetError> {
        Self::connect(scope, url, error_handler).build()
    }

    pub fn lazy<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> WebSocketBuilder<'scope> {
        Self::connect(scope, url, error_handler).auto_connect(false)
    }
}

#[derive(Copy, Clone)]
pub struct WebSocketConnection<'scope> {
    scope: Scope<'scope>,
    inner: StoredValue<'scope, WebSocketInner<'scope>>,
    state: ReadSignal<'scope, ConnectionState>,
    message: ReadSignal<'scope, Option<String>>,
    error: ReadSignal<'scope, Option<NetError>>,
    error_handler: ErrorReporter<'scope>,
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
        error: NetError,
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

fn create_socket(url: &str, protocols: &[String]) -> Result<JsWebSocket, NetError> {
    if protocols.is_empty() {
        JsWebSocket::new(url).map_err(NetError::from)
    } else {
        let protocols_value = js_sys::Array::new();
        for protocol in protocols {
            protocols_value.push(&wasm_bindgen::JsValue::from_str(protocol));
        }
        JsWebSocket::new_with_str_sequence(url, &protocols_value.into()).map_err(NetError::from)
    }
}

fn submit_completion<T: 'static>(
    token: &CompletionSender<T>,
    error_token: &CompletionSender<SilexError>,
    value: T,
    gate: Option<&Cell<bool>>,
) {
    let result = token.submit(value);
    let Err(error) = result else {
        return;
    };
    let error = match error {
        CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
        CallbackInvokeError::User(error) => error,
    };
    let error_result = catch_unwind(AssertUnwindSafe(|| error_token.submit(error)));
    if let Ok(Err(_)) | Err(_) = error_result {
        if let Some(gate) = gate {
            gate.set(false);
        }
        let _ = catch_unwind(AssertUnwindSafe(|| token.cancel()));
        let _ = catch_unwind(AssertUnwindSafe(|| error_token.cancel()));
        if let Err(panic) = error_result {
            resume_unwind(panic);
        }
    }
}

impl HostRegistration {
    fn new(
        socket: JsWebSocket,
        generation: u64,
        token: &CompletionSender<WebSocketEvent>,
        error_token: &CompletionSender<SilexError>,
    ) -> Self {
        let gate = Rc::new(Cell::new(true));

        let open_gate = gate.clone();
        let open_token = token.clone();
        let open_error_token = error_token.clone();
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            if open_gate.get() {
                submit_completion(
                    &open_token,
                    &open_error_token,
                    WebSocketEvent::Open { generation },
                    Some(&open_gate),
                );
            }
        }) as Box<dyn FnMut(Event)>);

        let message_gate = gate.clone();
        let message_token = token.clone();
        let message_error_token = error_token.clone();
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            if message_gate.get() {
                let Some(data) = event.data().as_string() else {
                    submit_completion(
                        &message_token,
                        &message_error_token,
                        WebSocketEvent::Error {
                            generation,
                            error: NetError::recoverable(NetErrorKind::JsError(
                                "WebSocket message data is not a string".to_string(),
                            )),
                        },
                        Some(&message_gate),
                    );
                    return;
                };
                submit_completion(
                    &message_token,
                    &message_error_token,
                    WebSocketEvent::Message { generation, data },
                    Some(&message_gate),
                );
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        let error_gate = gate.clone();
        let event_error_token = token.clone();
        let event_error_completion_token = error_token.clone();
        let on_error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
            if error_gate.get() {
                submit_completion(
                    &event_error_token,
                    &event_error_completion_token,
                    WebSocketEvent::Error {
                        generation,
                        error: NetError::recoverable(NetErrorKind::JsError(event.message())),
                    },
                    Some(&error_gate),
                );
            }
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

        let close_gate = gate.clone();
        let close_event_token = token.clone();
        let close_error_token = error_token.clone();
        let on_close = Closure::wrap(Box::new(move |event: web_sys::CloseEvent| {
            if close_gate.get() {
                submit_completion(
                    &close_event_token,
                    &close_error_token,
                    WebSocketEvent::Close {
                        generation,
                        code: event.code(),
                        reason: event.reason(),
                    },
                    Some(&close_gate),
                );
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

type DeferredCallback<'scope> = Box<dyn FnOnce() + 'scope>;

struct WebSocketInner<'scope> {
    url: ValueResolver<'scope>,
    protocols: Vec<String>,
    retry: Option<RetryPolicy>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(NetError) + 'scope>>,
    on_close: Vec<Rc<dyn Fn(u16, String) + 'scope>>,
    set_state: WriteSignal<'scope, ConnectionState>,
    state: ReadSignal<'scope, ConnectionState>,
    set_message: WriteSignal<'scope, Option<String>>,
    set_error: WriteSignal<'scope, Option<NetError>>,
    error_handler: ErrorReporter<'scope>,
    completion: CompletionSender<WebSocketEvent>,
    error_completion: CompletionSender<SilexError>,
    scope: Scope<'scope>,
    registration: Option<HostRegistration>,
    generation: u64,
    retry_generation: Option<u64>,
    retry_task: Option<TaskHandle<'scope>>,
    retry_attempt: u32,
    retry_started_at: Option<f64>,
    manual_close: bool,
}

fn cleanup_stored_inner<'scope>(
    inner: StoredValue<'scope, WebSocketInner<'scope>>,
) -> SilexResult<()> {
    inner.update(WebSocketInner::cleanup)?
}

impl Drop for WebSocketInner<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl<'scope> WebSocketInner<'scope> {
    fn cleanup(&mut self) -> SilexResult<()> {
        if let Some(task) = self.retry_task.take() {
            task.cancel();
        }
        self.retry_generation = None;
        self.generation = self.generation.wrapping_add(1);
        self.registration.take();
        self.completion.cancel();
        Ok(())
    }

    fn cancel_retry(&mut self) {
        if let Some(task) = self.retry_task.take() {
            task.cancel();
        }
        self.retry_generation = None;
    }

    fn defer_open(&self) -> DeferredCallback<'scope> {
        let handlers = self.on_open.clone();
        Box::new(move || {
            for handler in handlers {
                handler();
            }
        })
    }

    fn defer_error(&self, error: NetError) -> DeferredCallback<'scope> {
        let handlers = self.on_error.clone();
        Box::new(move || {
            for handler in handlers {
                handler(error.clone());
            }
        })
    }

    fn defer_close(&self, code: u16, reason: String) -> DeferredCallback<'scope> {
        let handlers = self.on_close.clone();
        Box::new(move || {
            for handler in handlers {
                handler(code, reason.clone());
            }
        })
    }

    fn reset_retry_window(&mut self) {
        self.retry_attempt = 0;
        self.retry_started_at = None;
    }

    fn try_open_current(
        &mut self,
    ) -> SilexResult<(Result<(), NetError>, Option<DeferredCallback<'scope>>)> {
        self.reset_retry_window();
        self.try_open_current_with_socket(None)
    }

    fn try_open_current_with_socket(
        &mut self,
        socket: Option<JsWebSocket>,
    ) -> SilexResult<(Result<(), NetError>, Option<DeferredCallback<'scope>>)> {
        self.cancel_retry();
        self.registration.take();
        self.manual_close = false;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.set_state.set(ConnectionState::Connecting)?;

        let socket = match socket {
            Some(socket) => Ok(socket),
            None => self
                .url
                .resolve()
                .map_err(NetError::from)
                .and_then(|url| create_socket(&url, &self.protocols)),
        };
        let socket = match socket {
            Ok(socket) => socket,
            Err(error) => {
                self.set_error.set(Some(error.clone()))?;
                self.set_state.set(ConnectionState::Error)?;
                return Ok((Err(error.clone()), Some(self.defer_error(error))));
            }
        };
        self.registration = Some(HostRegistration::new(
            socket,
            generation,
            &self.completion,
            &self.error_completion,
        ));
        Ok((Ok(()), None))
    }

    fn schedule_retry(&mut self, generation: u64) -> SilexResult<()> {
        if self.retry_generation == Some(generation) {
            return Ok(());
        }
        let Some(policy) = self.retry else {
            return Ok(());
        };
        let max_attempts = policy.max_attempts.max(1);
        self.retry_attempt = self.retry_attempt.saturating_add(1);
        if self.retry_attempt >= max_attempts {
            return Ok(());
        }

        let now = js_sys::Date::now();
        let started_at = *self.retry_started_at.get_or_insert(now);
        if let Some(max_elapsed) = policy.max_elapsed {
            let elapsed = Duration::from_millis((now - started_at).max(0.0) as u64);
            if elapsed >= max_elapsed {
                return Ok(());
            }
        }

        let delay = policy.delay_for_attempt(self.retry_attempt);
        let token = self.completion.clone();
        let error_token = self.error_completion.clone();
        self.retry_generation = Some(generation);
        self.retry_task = Some(self.scope.spawn_scoped(
            async move {
                if delay > Duration::from_millis(0) {
                    sleep(delay).await;
                }
                submit_completion(
                    &token,
                    &error_token,
                    WebSocketEvent::Retry { generation },
                    None,
                );
            },
            self.error_handler,
        )?);
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: WebSocketEvent,
    ) -> SilexResult<Option<DeferredCallback<'scope>>> {
        let mut callback = None;
        match event {
            WebSocketEvent::Open { generation }
                if generation == self.generation && !self.manual_close =>
            {
                self.retry_task.take();
                self.retry_generation = None;
                self.reset_retry_window();
                self.set_state.set(ConnectionState::Connected)?;
                self.set_error.set(None)?;
                callback = Some(self.defer_open());
            }
            WebSocketEvent::Message { generation, data }
                if generation == self.generation && !self.manual_close =>
            {
                self.set_message.set(Some(data))?;
                self.set_state.set(ConnectionState::Connected)?;
            }
            WebSocketEvent::Error { generation, error }
                if generation == self.generation && !self.manual_close =>
            {
                self.set_error.set(Some(error.clone()))?;
                self.set_state.set(ConnectionState::Error)?;
                self.schedule_retry(generation)?;
                callback = Some(self.defer_error(error));
            }
            WebSocketEvent::Close {
                generation,
                code,
                reason,
            } if generation == self.generation => {
                self.registration.take();
                self.set_state.set(ConnectionState::Closed)?;
                let manual_close = self.manual_close;
                self.manual_close = false;
                if !manual_close {
                    self.schedule_retry(generation)?;
                }
                callback = Some(self.defer_close(code, reason));
            }
            WebSocketEvent::Retry { generation } if generation == self.generation => {
                self.retry_task.take();
                self.retry_generation = None;
                if let Some(policy) = self.retry
                    && let Some(started_at) = self.retry_started_at
                {
                    let elapsed =
                        Duration::from_millis((js_sys::Date::now() - started_at).max(0.0) as u64);
                    if policy.max_elapsed.is_some_and(|limit| elapsed >= limit) {
                        return Ok(callback);
                    }
                }
                let (_, retry_callback) = self.try_open_current_with_socket(None)?;
                callback = retry_callback;
            }
            _ => {}
        }
        Ok(callback)
    }

    fn close(&mut self) -> SilexResult<()> {
        self.cancel_retry();
        if self.manual_close {
            return Ok(());
        }
        self.manual_close = true;
        if let Some(registration) = &self.registration {
            self.set_state.set(ConnectionState::Closing)?;
            let _ = registration.socket.close();
        } else {
            self.set_state.set(ConnectionState::Closed)?;
        }
        Ok(())
    }
}

impl<'scope> WebSocketConnection<'scope> {
    fn memo_state<T: PartialEq + 'scope>(
        &self,
        f: impl Fn(ConnectionState) -> T + 'scope,
    ) -> SilexResult<Rx<'scope, T>> {
        let state = self.state;
        let handler = self.error_handler;
        self.scope
            .memo(move |_| state.get().map(&f), handler)
            .map(|memo| memo.into_rx())
    }

    pub fn is_connected(&self) -> SilexResult<Rx<'scope, bool>> {
        self.memo_state(|state| state.is_connected())
    }

    pub fn is_connecting(&self) -> SilexResult<Rx<'scope, bool>> {
        self.memo_state(|state| matches!(state, ConnectionState::Connecting))
    }

    pub fn is_closed(&self) -> SilexResult<Rx<'scope, bool>> {
        self.memo_state(|state| {
            matches!(
                state,
                ConnectionState::Closed | ConnectionState::Disconnected
            )
        })
    }

    pub fn state(&self) -> ReadSignal<'scope, ConnectionState> {
        self.state
    }

    #[cfg(feature = "json")]
    pub fn message<T>(&self) -> SilexResult<Rx<'scope, Option<T>>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let message = self.message;
        let handler = self.error_handler;
        self.scope
            .memo(
                move |_| {
                    message
                        .get()?
                        .map(|raw| {
                            serde_json::from_str(&raw).map_err(|error| {
                                NetError::recoverable(NetErrorKind::DecodeError(format!(
                                    "decode WebSocket message failed: {error}"
                                )))
                                .into()
                            })
                        })
                        .transpose()
                },
                handler,
            )
            .map(|memo| memo.into_rx())
    }

    pub fn state_str(&self) -> SilexResult<Rx<'scope, &'scope str>> {
        self.memo_state(|state| state.as_str())
    }

    pub fn raw_message(&self) -> ReadSignal<'scope, Option<String>> {
        self.message
    }

    /// Return the latest typed connection error, if one was reported.
    pub fn error(&self) -> ReadSignal<'scope, Option<NetError>> {
        self.error
    }

    pub fn send(&self, value: impl Into<String>) -> Result<(), NetError> {
        let message = value.into();
        self.inner
            .with(|inner| {
                let state = inner.state.get().map_err(NetError::from)?;
                if matches!(state, ConnectionState::Closed) {
                    return Err(NetError::recoverable(NetErrorKind::ConnectionClosed));
                }
                if !matches!(state, ConnectionState::Connected) {
                    return Err(NetError::recoverable(NetErrorKind::ConnectionNotReady {
                        state,
                    }));
                }
                let Some(registration) = &inner.registration else {
                    return Err(NetError::recoverable(NetErrorKind::ConnectionClosed));
                };
                match registration.socket.ready_state() {
                    JsWebSocket::OPEN => registration
                        .socket
                        .send_with_str(&message)
                        .map_err(NetError::from),
                    JsWebSocket::CONNECTING => {
                        Err(NetError::recoverable(NetErrorKind::ConnectionNotReady {
                            state: ConnectionState::Connecting,
                        }))
                    }
                    JsWebSocket::CLOSING => {
                        Err(NetError::recoverable(NetErrorKind::ConnectionNotReady {
                            state: ConnectionState::Closing,
                        }))
                    }
                    JsWebSocket::CLOSED => {
                        Err(NetError::recoverable(NetErrorKind::ConnectionClosed))
                    }
                    _ => Err(NetError::recoverable(NetErrorKind::ConnectionNotReady {
                        state,
                    })),
                }
            })
            .map_err(NetError::from)
            .and_then(|result| result)
    }

    pub fn send_text(&self, value: impl Into<String>) -> Result<(), NetError> {
        self.send(value)
    }

    #[cfg(feature = "json")]
    pub fn send_json<T>(&self, value: &T) -> Result<(), NetError>
    where
        T: serde::Serialize,
    {
        let payload = serde_json::to_string(value).map_err(|error| {
            NetError::recoverable(NetErrorKind::SerializeError(error.to_string()))
        })?;
        self.send(payload)
    }

    pub fn close(&self) -> Result<(), NetError> {
        self.inner
            .update(WebSocketInner::close)
            .map_err(NetError::from)??;
        Ok(())
    }

    pub fn reconnect(&self) -> Result<(), NetError> {
        if !matches!(
            self.state.get().map_err(NetError::from)?,
            ConnectionState::Closed | ConnectionState::Disconnected | ConnectionState::Error
        ) {
            return Ok(());
        }
        let (result, callbacks) = self
            .inner
            .update(WebSocketInner::try_open_current)
            .map_err(NetError::from)??;
        if let Some(callback) = callbacks {
            callback();
        }
        result
    }

    pub fn toggle(&self) -> Result<(), NetError> {
        if self.state.get().map_err(NetError::from)?.is_active() {
            self.close()?;
        } else {
            self.reconnect()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct WebSocketBuilder<'scope> {
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    url: ValueResolver<'scope>,
    protocols: Vec<String>,
    auto_connect: bool,
    reconnect: Option<RetryPolicy>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(NetError) + 'scope>>,
    on_close: Vec<Rc<dyn Fn(u16, String) + 'scope>>,
}

impl<'scope> WebSocketBuilder<'scope> {
    fn new(
        scope: Scope<'scope>,
        url: ValueResolver<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> Self {
        Self {
            scope,
            error_handler,
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

    pub fn reconnect(self, attempts: u32, delay: Duration) -> Self {
        self.reconnect_policy(RetryPolicy::new(attempts, delay))
    }

    /// Configure the owner-bound retry policy used after an error or close.
    pub fn reconnect_policy(mut self, policy: RetryPolicy) -> Self {
        self.reconnect = Some(policy);
        self
    }

    pub fn on_open(mut self, handler: impl Fn() + 'scope) -> Self {
        self.on_open.push(Rc::new(handler));
        self
    }

    pub fn on_error(mut self, handler: impl Fn(NetError) + 'scope) -> Self {
        self.on_error.push(Rc::new(handler));
        self
    }

    pub fn on_close(mut self, handler: impl Fn(u16, String) + 'scope) -> Self {
        self.on_close.push(Rc::new(handler));
        self
    }

    pub fn build(self) -> Result<WebSocketConnection<'scope>, NetError> {
        let Self {
            scope,
            error_handler,
            url,
            protocols,
            auto_connect,
            reconnect,
            on_open,
            on_error,
            on_close,
        } = self;
        url.validate_runtime(scope).map_err(NetError::from)?;
        let initial_socket = if auto_connect {
            match url
                .resolve()
                .map_err(NetError::from)
                .and_then(|url| create_socket(&url, &protocols))
            {
                Ok(socket) => Some(socket),
                Err(error) => {
                    for handler in &on_error {
                        handler(error.clone());
                    }
                    return Err(error);
                }
            }
        } else {
            None
        };

        let (state, set_state) = scope.signal(if auto_connect {
            ConnectionState::Connecting
        } else {
            ConnectionState::Disconnected
        })?;
        let (message, set_message) = scope.signal(None::<String>)?;
        let (error, set_error) = scope.signal(None::<NetError>)?;
        let inner_slot = Rc::new(Cell::new(
            None::<StoredValue<'scope, WebSocketInner<'scope>>>,
        ));
        let inner_slot_for_completion = inner_slot.clone();
        let error_completion = scope.completion_sender(unwind_safe(move |error: SilexError| {
            error_handler.handle(error).map_err(SilexError::fatal)
        }))?;
        let completion = scope.completion_sender(unwind_safe(move |event: WebSocketEvent| {
            if let Some(inner) = inner_slot_for_completion.get() {
                let callback = inner.update(|inner| inner.handle_event(event))??;
                if let Some(callback) = callback {
                    callback();
                }
            }
            Ok(())
        }))?;
        let inner = scope.stored(WebSocketInner {
            url,
            protocols,
            retry: reconnect,
            on_open,
            on_error,
            on_close,
            set_state,
            state,
            set_message,
            set_error,
            error_handler,
            completion,
            error_completion,
            scope,
            registration: None,
            generation: 0,
            retry_generation: None,
            retry_task: None,
            retry_attempt: 0,
            retry_started_at: None,
            manual_close: false,
        })?;
        inner_slot.set(Some(inner));
        scope
            .on_cleanup(
                move || -> SilexResult<()> { cleanup_stored_inner(inner) },
                error_handler,
            )
            .map_err(NetError::from)?;

        let connection = WebSocketConnection {
            scope,
            inner,
            state,
            message,
            error,
            error_handler,
        };
        if auto_connect {
            let (result, callbacks) = inner
                .update(|inner| inner.try_open_current_with_socket(initial_socket))
                .map_err(NetError::from)??;
            if let Some(callback) = callbacks {
                callback();
            }
            if let Err(error) = result {
                let _ = cleanup_stored_inner(inner);
                return Err(error);
            }
        }
        Ok(connection)
    }
}
