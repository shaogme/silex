use crate::net::NetError;
use crate::net::state::{ConnectionState, EventMessage, HttpResponse, RequestBody, RequestSpec};
use crate::net::state::RetryPolicy;
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use js_sys::Function;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AbortController, Event, EventSource as JsEventSource, FormData, Headers, MessageEvent, Request,
    RequestInit, Response, WebSocket as JsWebSocket,
};

#[cfg(feature = "json")]
use silex_core::reactivity::Memo;
use silex_core::reactivity::{ReadSignal, Signal};
#[cfg(feature = "json")]
use silex_core::traits::RxGet;
use silex_core::traits::RxWrite;

pub type TransportFuture<'a> = Pin<Box<dyn Future<Output = Result<HttpResponse, NetError>> + 'a>>;

pub trait Transport: 'static {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BrowserTransport;

#[derive(Clone, Copy, Debug, Default)]
pub struct HttpBackend;

impl Transport for BrowserTransport {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        Box::pin(async move { Self::send(spec).await })
    }
}

impl Transport for HttpBackend {
    fn send(&self, spec: RequestSpec) -> TransportFuture<'_> {
        Box::pin(async move { Self::send(spec).await })
    }
}

impl HttpBackend {
    pub async fn send(spec: RequestSpec) -> Result<HttpResponse, NetError> {
        BrowserTransport::send(spec).await
    }
}

impl BrowserTransport {
    pub async fn send(spec: RequestSpec) -> Result<HttpResponse, NetError> {
        let window = web_sys::window().ok_or(NetError::BrowserUnavailable)?;

        let init = RequestInit::new();
        init.set_method(spec.method.as_str());

        let headers = Headers::new().map_err(NetError::from)?;
        for (name, value) in &spec.headers {
            headers
                .append(name, value)
                .map_err(|err| NetError::JsError(format!("{err:?}")))?;
        }
        init.set_headers(headers.as_ref());

        let mut timeout_handle: Option<i32> = None;
        let mut timeout_guard: Option<Closure<dyn FnMut()>> = None;
        let timed_out = Rc::new(Cell::new(false));

        if let Some(timeout) = spec.timeout {
            let controller = AbortController::new().map_err(NetError::from)?;
            let signal = controller.signal();
            init.set_signal(Some(&signal));

            let timed_out_flag = timed_out.clone();
            let abort_controller = controller.clone();
            let timeout_closure = Closure::wrap(Box::new(move || {
                timed_out_flag.set(true);
                abort_controller.abort();
            }) as Box<dyn FnMut()>);

            let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    timeout_closure.as_ref().unchecked_ref(),
                    millis,
                )
                .map_err(NetError::from)?;

            timeout_handle = Some(handle);
            timeout_guard = Some(timeout_closure);
        }

        match &spec.body {
            RequestBody::Empty => {}
            RequestBody::Text(text) | RequestBody::Json(text) => {
                let body = JsValue::from_str(text);
                init.set_body(&body);
            }
            RequestBody::Form(fields) => {
                let form = FormData::new().map_err(NetError::from)?;
                for (name, value) in fields {
                    form.append_with_str(name, value)
                        .map_err(NetError::from)?;
                }
                init.set_body(form.as_ref());
            }
        }

        let request = Request::new_with_str_and_init(&spec.url, &init).map_err(NetError::from)?;
        let response_value = JsFuture::from(window.fetch_with_request(&request)).await;

        if let Some(handle) = timeout_handle {
            window.clear_timeout_with_handle(handle);
        }
        drop(timeout_guard);

        let response_value = match response_value {
            Ok(value) => value,
            Err(_err) => {
                return Err(if timed_out.get() {
                    NetError::Timeout
                } else {
                    NetError::TransportUnavailable
                });
            }
        };

        let response: Response = response_value
            .dyn_into()
            .map_err(|err| NetError::JsError(format!("{err:?}")))?;

        let raw_body = JsFuture::from(response.text().map_err(NetError::from)?)
            .await
            .map_err(NetError::from)?
            .as_string()
            .unwrap_or_default();

        let status = response.status();
        let status_text = response.status_text();
        let url = response.url();

        if !response.ok() {
            return Err(NetError::HttpStatus {
                status,
                body: raw_body,
            });
        }

        Ok(HttpResponse {
            url,
            status,
            status_text,
            raw_body,
        })
    }
}

pub struct WebSocket;

impl WebSocket {
    pub fn connect(url: impl Into<String>) -> WebSocketBuilder {
        WebSocketBuilder::new(url)
    }
}

pub struct EventStream;

impl EventStream {
    pub fn new(url: impl Into<String>) -> EventStreamBuilder {
        EventStreamBuilder::new(url)
    }
}

pub struct WebSocketConnection {
    socket: JsWebSocket,
    pub state: ReadSignal<ConnectionState>,
    pub message: ReadSignal<Option<String>>,
    pub error: ReadSignal<Option<String>>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    _on_close: Closure<dyn FnMut(web_sys::CloseEvent)>,
}

impl Drop for WebSocketConnection {
    fn drop(&mut self) {
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}


impl WebSocketConnection {
    pub fn state(&self) -> ReadSignal<ConnectionState> {
        self.state
    }

    #[cfg(feature = "json")]
    pub fn message<T>(&self) -> Memo<Option<T>>
    where
        T: serde::de::DeserializeOwned + Clone + PartialEq + 'static,
    {
        let message = self.message;
        Memo::new(move |_| {
            message
                .get()
                .and_then(|raw| serde_json_wasm::from_str(&raw).ok())
        })
    }

    pub fn raw_message(&self) -> ReadSignal<Option<String>> {
        self.message
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    pub fn send(&self, value: impl Into<String>) -> Result<(), NetError> {
        self.socket
            .send_with_str(&value.into())
            .map_err(NetError::from)
    }

    #[cfg(feature = "json")]
    pub fn send_json<T>(&self, value: &T) -> Result<(), NetError>
    where
        T: serde::Serialize,
    {
        let payload = serde_json_wasm::to_string(value)
            .map_err(|err| NetError::SerializeError(err.to_string()))?;
        self.send(payload)
    }

    pub fn close(&self) -> Result<(), NetError> {
        self.socket.close().map_err(NetError::from)
    }
}

#[derive(Clone)]
pub struct WebSocketBuilder {
    pub(crate) url: String,
    pub(crate) protocols: Vec<String>,
    pub(crate) reconnect: Option<RetryPolicy>,
    pub(crate) on_open: Vec<Rc<dyn Fn()>>,
    pub(crate) on_error: Vec<Rc<dyn Fn(String)>>,
    pub(crate) on_close: Vec<Rc<dyn Fn(u16, String)>>,
}

impl WebSocketBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            protocols: Vec::new(),
            reconnect: None,
            on_open: Vec::new(),
            on_error: Vec::new(),
            on_close: Vec::new(),
        }
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocols.push(protocol.into());
        self
    }

    pub fn reconnect(mut self, attempts: u32, delay: std::time::Duration) -> Self {
        self.reconnect = Some(RetryPolicy::new(attempts, delay));
        self
    }

    pub fn on_open(mut self, f: impl Fn() + 'static) -> Self {
        self.on_open.push(Rc::new(f));
        self
    }

    pub fn on_error(mut self, f: impl Fn(String) + 'static) -> Self {
        self.on_error.push(Rc::new(f));
        self
    }

    pub fn on_close(mut self, f: impl Fn(u16, String) + 'static) -> Self {
        self.on_close.push(Rc::new(f));
        self
    }

    pub fn build(self) -> WebSocketConnection {
        let socket = if self.protocols.is_empty() {
            JsWebSocket::new(&self.url).expect("failed to create WebSocket")
        } else {
            let protocols = js_sys::Array::new();
            for protocol in &self.protocols {
                protocols.push(&wasm_bindgen::JsValue::from_str(protocol));
            }
            let protocols = protocols.into();
            JsWebSocket::new_with_str_sequence(&self.url, &protocols)
                .expect("failed to create WebSocket")
        };

        let (state, set_state) = Signal::new(ConnectionState::Connecting);
        let (message, set_message) = Signal::new(None::<String>);
        let (error, set_error) = Signal::new(None::<String>);

        let on_open_handlers = self.on_open.clone();
        let on_error_handlers = self.on_error.clone();
        let on_close_handlers = self.on_close.clone();
        let reconnect = self.reconnect;

        let state_for_open = set_state;
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            state_for_open.set(ConnectionState::Connected);
            for handler in &on_open_handlers {
                handler();
            }
        }) as Box<dyn FnMut(Event)>);

        let state_for_message = set_state;
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data().as_string().unwrap_or_default();
            set_message.set(Some(data));
            state_for_message.set(ConnectionState::Connected);
        }) as Box<dyn FnMut(MessageEvent)>);

        let state_for_error = set_state;
        let on_error = Closure::wrap(Box::new(move |event: web_sys::ErrorEvent| {
            let message = event.message();
            set_error.set(Some(message.clone()));
            state_for_error.set(ConnectionState::Error);
            for handler in &on_error_handlers {
                handler(message.clone());
            }
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

        let state_for_close = set_state;
        let on_close = Closure::wrap(Box::new(move |event: web_sys::CloseEvent| {
            state_for_close.set(ConnectionState::Closed);
            let reason = event.reason();
            for handler in &on_close_handlers {
                handler(event.code(), reason.clone());
            }
            let _ = reconnect;
        }) as Box<dyn FnMut(web_sys::CloseEvent)>);

        socket.set_onopen(Some(on_open.as_ref().unchecked_ref::<Function>()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref::<Function>()));
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref::<Function>()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref::<Function>()));

        WebSocketConnection {
            socket,
            state,
            message,
            error,
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
            _on_close: on_close,
        }
    }
}

pub struct EventStreamConnection {
    source: JsEventSource,
    pub state: ReadSignal<ConnectionState>,
    pub messages: ReadSignal<Vec<EventMessage>>,
    pub error: ReadSignal<Option<String>>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(Event)>,
    event_name: Option<String>,
}

impl Drop for EventStreamConnection {
    fn drop(&mut self) {
        self.source.set_onopen(None);
        self.source.set_onerror(None);
        if let Some(name) = &self.event_name {
            let _ = self.source.remove_event_listener_with_callback(
                name,
                self._on_message.as_ref().unchecked_ref(),
            );
        } else {
            self.source.set_onmessage(None);
        }
        self.source.close();
    }
}


impl EventStreamConnection {
    pub fn state(&self) -> ReadSignal<ConnectionState> {
        self.state
    }

    #[cfg(feature = "json")]
    pub fn messages<T>(&self) -> Memo<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Clone + PartialEq + 'static,
    {
        let messages = self.messages;
        Memo::new(move |_| {
            messages
                .get()
                .into_iter()
                .filter_map(|msg| serde_json_wasm::from_str(&msg.data).ok())
                .collect()
        })
    }

    #[cfg(feature = "json")]
    pub fn last_message<T>(&self) -> Memo<Option<T>>
    where
        T: serde::de::DeserializeOwned + Clone + PartialEq + 'static,
    {
        let messages = self.messages;
        Memo::new(move |_| {
            messages
                .get()
                .last()
                .and_then(|msg| serde_json_wasm::from_str(&msg.data).ok())
        })
    }

    pub fn raw_messages(&self) -> ReadSignal<Vec<EventMessage>> {
        self.messages
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    pub fn close(&self) {
        self.source.close();
    }
}

#[derive(Clone)]
pub struct EventStreamBuilder {
    pub(crate) url: String,
    pub(crate) event_name: Option<String>,
    pub(crate) on_open: Vec<Rc<dyn Fn()>>,
    pub(crate) on_error: Vec<Rc<dyn Fn(String)>>,
}

impl EventStreamBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            event_name: None,
            on_open: Vec::new(),
            on_error: Vec::new(),
        }
    }

    pub fn event(mut self, name: impl Into<String>) -> Self {
        self.event_name = Some(name.into());
        self
    }

    pub fn on_open(mut self, f: impl Fn() + 'static) -> Self {
        self.on_open.push(Rc::new(f));
        self
    }

    pub fn on_error(mut self, f: impl Fn(String) + 'static) -> Self {
        self.on_error.push(Rc::new(f));
        self
    }

    pub fn build(self) -> EventStreamConnection {
        let source = JsEventSource::new(&self.url).expect("failed to create EventSource");

        let (state, set_state) = Signal::new(ConnectionState::Connecting);
        let (messages, set_messages) = Signal::new(Vec::<EventMessage>::new());
        let (error, set_error) = Signal::new(None::<String>);

        let on_open_handlers = self.on_open.clone();
        let on_error_handlers = self.on_error.clone();

        let state_for_open = set_state;
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            state_for_open.set(ConnectionState::Connected);
            for handler in &on_open_handlers {
                handler();
            }
        }) as Box<dyn FnMut(Event)>);

        let state_for_message = set_state;
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data().as_string().unwrap_or_default();
            let event_name = event.type_();
            set_messages.update(|messages: &mut Vec<EventMessage>| {
                messages.push(EventMessage {
                    event: Some(event_name),
                    data,
                });
            });
            state_for_message.set(ConnectionState::Connected);
        }) as Box<dyn FnMut(MessageEvent)>);

        let state_for_error = set_state;
        let on_error = Closure::wrap(Box::new(move |_event: Event| {
            state_for_error.set(ConnectionState::Error);
            set_error.set(Some("event stream error".to_string()));
            for handler in &on_error_handlers {
                handler("event stream error".to_string());
            }
        }) as Box<dyn FnMut(Event)>);

        source.set_onopen(Some(on_open.as_ref().unchecked_ref::<Function>()));
        if let Some(event_name) = &self.event_name {
            let on_message_fn = on_message.as_ref().unchecked_ref::<Function>();
            source
                .add_event_listener_with_callback(&event_name, on_message_fn)
                .expect("failed to register event listener");
        } else {
            source.set_onmessage(Some(on_message.as_ref().unchecked_ref::<Function>()));
        }
        source.set_onerror(Some(on_error.as_ref().unchecked_ref::<Function>()));

        EventStreamConnection {
            source,
            state,
            messages,
            error,
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
            event_name: self.event_name,
        }
    }
}
