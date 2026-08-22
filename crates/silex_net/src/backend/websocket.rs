use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    time::Duration,
};

use gloo_timers::future::sleep;
use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, MessageEvent, WebSocket as JsWebSocket};

use silex_core::{
    CallbackInvokeError, CompletionSender, ErrorHandlerAnchor, ErrorHandlerInput, ErrorReporter,
    OwnerAccess, ReactiveError, ReadSignal, Rx, RxGet, RxRead, Signal, SilexError, SilexErrorKind,
    SilexResult, StoredValue, TaskHandle, unwind_safe,
};

use crate::{
    NetError, NetErrorKind,
    builder::{IntoNetValue, ValueResolver},
    operation::{ConnectionDriver, OperationId},
    state::{ConnectionState, RetryPolicy},
};

pub struct WebSocket;

impl WebSocket {
    pub fn connect<'scope, H>(
        scope: OwnerAccess<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: H,
    ) -> WebSocketBuilder<'scope, H>
    where
        H: ErrorHandlerInput<'scope>,
    {
        WebSocketBuilder::new(scope, url.into_net_value(), error_handler)
    }

    pub fn open<'scope, H>(
        scope: OwnerAccess<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: H,
    ) -> Result<WebSocketConnection<'scope>, NetError>
    where
        H: ErrorHandlerInput<'scope>,
    {
        Self::connect(scope, url, error_handler).build()
    }

    pub fn lazy<'scope, H>(
        scope: OwnerAccess<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: H,
    ) -> WebSocketBuilder<'scope, H>
    where
        H: ErrorHandlerInput<'scope>,
    {
        Self::connect(scope, url, error_handler).auto_connect(false)
    }
}

#[derive(Copy, Clone)]
pub struct WebSocketConnection<'scope> {
    scope: OwnerAccess<'scope>,
    inner: StoredValue<'scope, WebSocketInner<'scope>>,
    state: ReadSignal<'scope, ConnectionState>,
    message: ReadSignal<'scope, Option<String>>,
    error: ReadSignal<'scope, Option<NetError>>,
    error_handler: ErrorReporter<'scope>,
}

#[derive(Clone)]
enum WebSocketEvent {
    Open {
        operation: OperationId,
    },
    Message {
        operation: OperationId,
        data: String,
    },
    Error {
        operation: OperationId,
        error: NetError,
    },
    Close {
        operation: OperationId,
        code: u16,
        reason: String,
    },
    Retry {
        operation: OperationId,
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
    let (callback, close) = error.into_parts();
    let mut failures = false;
    let mut submit_error =
        |error| match catch_unwind(AssertUnwindSafe(|| error_token.submit(error))) {
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => failures = true,
        };
    if let Some(callback) = callback {
        submit_error(match callback {
            CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
            CallbackInvokeError::User(error) => error,
            CallbackInvokeError::Handler(error) => SilexError::fatal(ReactiveError::Handler(error)),
        });
    }
    if let Some(close) = close {
        submit_error(SilexError::fatal(SilexErrorKind::Close(close)));
    }
    if failures {
        if let Some(gate) = gate {
            gate.set(false);
        }
        let _ = catch_unwind(AssertUnwindSafe(|| token.cancel()));
        let _ = catch_unwind(AssertUnwindSafe(|| error_token.cancel()));
    }
}

impl HostRegistration {
    fn new(
        socket: JsWebSocket,
        operation: OperationId,
        token: &CompletionSender<WebSocketEvent>,
        error_token: &CompletionSender<SilexError>,
    ) -> Self {
        let gate = Rc::new(Cell::new(true));

        let open_gate = gate.clone();
        let open_token = token.clone();
        let open_error_token = error_token.clone();
        let on_open = Closure::wrap_assert_unwind_safe(Box::new(move |_event: Event| {
            if open_gate.get() {
                submit_completion(
                    &open_token,
                    &open_error_token,
                    WebSocketEvent::Open { operation },
                    Some(&open_gate),
                );
            }
        }) as Box<dyn FnMut(Event)>);

        let message_gate = gate.clone();
        let message_token = token.clone();
        let message_error_token = error_token.clone();
        let on_message = Closure::wrap_assert_unwind_safe(Box::new(move |event: MessageEvent| {
            if message_gate.get() {
                let Some(data) = event.data().as_string() else {
                    submit_completion(
                        &message_token,
                        &message_error_token,
                        WebSocketEvent::Error {
                            operation,
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
                    WebSocketEvent::Message { operation, data },
                    Some(&message_gate),
                );
            }
        })
            as Box<dyn FnMut(MessageEvent)>);

        let error_gate = gate.clone();
        let event_error_token = token.clone();
        let event_error_completion_token = error_token.clone();
        let on_error =
            Closure::wrap_assert_unwind_safe(Box::new(move |event: web_sys::ErrorEvent| {
                if error_gate.get() {
                    submit_completion(
                        &event_error_token,
                        &event_error_completion_token,
                        WebSocketEvent::Error {
                            operation,
                            error: NetError::recoverable(NetErrorKind::JsError(event.message())),
                        },
                        Some(&error_gate),
                    );
                }
            }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

        let close_gate = gate.clone();
        let close_event_token = token.clone();
        let close_error_token = error_token.clone();
        let on_close =
            Closure::wrap_assert_unwind_safe(Box::new(move |event: web_sys::CloseEvent| {
                if close_gate.get() {
                    submit_completion(
                        &close_event_token,
                        &close_error_token,
                        WebSocketEvent::Close {
                            operation,
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
    state: Signal<'scope, ConnectionState>,
    message: Signal<'scope, Option<String>>,
    error: Signal<'scope, Option<NetError>>,
    error_handler_owner: ErrorHandlerAnchor<'scope>,
    completion: CompletionSender<WebSocketEvent>,
    error_completion: CompletionSender<SilexError>,
    scope: OwnerAccess<'scope>,
    registration: Option<HostRegistration>,
    driver: ConnectionDriver,
    retry_task: Option<TaskHandle<'scope>>,
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
        self.driver.close();
        self.registration.take();
        self.completion
            .cancel()
            .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))?;
        Ok(())
    }

    fn cancel_retry(&mut self) {
        if let Some(task) = self.retry_task.take() {
            task.cancel();
        }
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
        self.driver.reset_retry_window();
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
        let operation = self.driver.begin()?;
        self.state.set(ConnectionState::Connecting)?;

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
                self.error.set(Some(error.clone()))?;
                self.state.set(ConnectionState::Error)?;
                return Ok((Err(error.clone()), Some(self.defer_error(error))));
            }
        };
        self.registration = Some(HostRegistration::new(
            socket,
            operation,
            &self.completion,
            &self.error_completion,
        ));
        Ok((Ok(()), None))
    }

    fn schedule_retry(&mut self, operation: OperationId) -> SilexResult<()> {
        let Some(policy) = self.retry else {
            return Ok(());
        };
        let Some((_, delay)) = self.driver.next_current_retry(policy, js_sys::Date::now()) else {
            return Ok(());
        };
        let token = self.completion.clone();
        let error_token = self.error_completion.clone();
        self.retry_task = Some(self.scope.spawn_scoped(
            async move {
                if delay > Duration::from_millis(0) {
                    sleep(delay).await;
                }
                submit_completion(
                    &token,
                    &error_token,
                    WebSocketEvent::Retry { operation },
                    None,
                );
            },
            self.error_handler_owner.view(),
        )?);
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: WebSocketEvent,
    ) -> SilexResult<Option<DeferredCallback<'scope>>> {
        let mut callback = None;
        match event {
            WebSocketEvent::Open { operation }
                if self.driver.is_current(operation) && !self.manual_close =>
            {
                self.retry_task.take();
                self.reset_retry_window();
                self.state.set(ConnectionState::Connected)?;
                self.error.set(None)?;
                callback = Some(self.defer_open());
            }
            WebSocketEvent::Message { operation, data }
                if self.driver.is_current(operation) && !self.manual_close =>
            {
                self.message.set(Some(data))?;
                self.state.set(ConnectionState::Connected)?;
            }
            WebSocketEvent::Error { operation, error }
                if self.driver.is_current(operation) && !self.manual_close =>
            {
                self.error.set(Some(error.clone()))?;
                self.state.set(ConnectionState::Error)?;
                if self.driver.consume_current_failure() {
                    self.schedule_retry(operation)?;
                }
                callback = Some(self.defer_error(error));
            }
            WebSocketEvent::Close {
                operation,
                code,
                reason,
            } if self.driver.is_current(operation) => {
                self.registration.take();
                self.state.set(ConnectionState::Closed)?;
                let manual_close = self.manual_close;
                self.manual_close = false;
                if !manual_close && self.driver.consume_current_failure() {
                    self.schedule_retry(operation)?;
                }
                callback = Some(self.defer_close(code, reason));
            }
            WebSocketEvent::Retry { operation } if self.driver.is_current(operation) => {
                self.retry_task.take();
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
            self.state.set(ConnectionState::Closing)?;
            let _ = registration.socket.close();
        } else {
            self.state.set(ConnectionState::Closed)?;
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
            .computed(move || state.get().map(&f), handler)
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
            .computed(
                move || {
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
pub struct WebSocketBuilder<'scope, H = ErrorReporter<'scope>> {
    scope: OwnerAccess<'scope>,
    error_handler: H,
    url: ValueResolver<'scope>,
    protocols: Vec<String>,
    auto_connect: bool,
    reconnect: Option<RetryPolicy>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(NetError) + 'scope>>,
    on_close: Vec<Rc<dyn Fn(u16, String) + 'scope>>,
}

impl<'scope, H> WebSocketBuilder<'scope, H> {
    fn new(scope: OwnerAccess<'scope>, url: ValueResolver<'scope>, error_handler: H) -> Self {
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

    pub fn build(self) -> Result<WebSocketConnection<'scope>, NetError>
    where
        H: ErrorHandlerInput<'scope>,
    {
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
        let error_handler = error_handler.handler_ref();
        let handler_anchor = error_handler
            .anchor()
            .map_err(|error| NetError::from(SilexError::fatal(ReactiveError::Handler(error))))?;
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

        let state = scope.signal(if auto_connect {
            ConnectionState::Connecting
        } else {
            ConnectionState::Disconnected
        })?;
        let message = scope.signal(None::<String>)?;
        let error = scope.signal(None::<NetError>)?;
        let inner_slot = Rc::new(Cell::new(
            None::<StoredValue<'scope, WebSocketInner<'scope>>>,
        ));
        let inner_slot_for_completion = inner_slot.clone();
        let error_lease = error_handler
            .lease()
            .map_err(|error| NetError::from(SilexError::fatal(ReactiveError::Handler(error))))?;
        let error_completion_lease = error_lease.clone();
        let error_completion = scope.completion_sender(unwind_safe(move |error: SilexError| {
            error_completion_lease
                .handle(error)
                .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))
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
            state,
            message,
            error,
            error_handler_owner: handler_anchor.clone(),
            completion,
            error_completion,
            scope,
            registration: None,
            driver: ConnectionDriver::new(),
            retry_task: None,
            manual_close: false,
        })?;
        inner_slot.set(Some(inner));
        scope
            .on_cleanup(
                move || -> SilexResult<()> { cleanup_stored_inner(inner) },
                handler_anchor,
            )
            .map_err(NetError::from)?;

        let connection = WebSocketConnection {
            scope,
            inner,
            state: state.read_signal(),
            message: message.read_signal(),
            error: error.read_signal(),
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
