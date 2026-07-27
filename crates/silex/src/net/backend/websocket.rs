use std::rc::Rc;

use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, MessageEvent, WebSocket as JsWebSocket};

use silex_core::{
    reactivity::{Memo, ReadSignal, Signal, StoredValue, WriteSignal},
    traits::{RxGet, RxRead, RxWrite},
};

use crate::net::{
    NetError,
    state::{ConnectionState, RetryPolicy},
};

pub struct WebSocket;

impl WebSocket {
    pub fn connect(url: impl Into<String>) -> WebSocketBuilder {
        WebSocketBuilder::new(url)
    }

    pub fn open(url: impl Into<String>) -> WebSocketConnection {
        Self::connect(url).build()
    }

    pub fn lazy(url: impl Into<String>) -> WebSocketConnection {
        Self::connect(url).auto_connect(false).build()
    }
}

#[derive(Copy, Clone)]
pub struct WebSocketConnection {
    inner: StoredValue<WebSocketInner>,
    state: ReadSignal<ConnectionState>,
    message: ReadSignal<Option<String>>,
    error: ReadSignal<Option<String>>,
}

struct WebSocketInner {
    socket: Option<JsWebSocket>,
    url: String,
    protocols: Vec<String>,
    set_state: WriteSignal<ConnectionState>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
    _on_close: Closure<dyn FnMut(web_sys::CloseEvent)>,
}

impl Drop for WebSocketInner {
    fn drop(&mut self) {
        if let Some(socket) = &self.socket {
            socket.set_onopen(None);
            socket.set_onmessage(None);
            socket.set_onerror(None);
            socket.set_onclose(None);
            let _ = socket.close();
        }
    }
}

impl WebSocketConnection {
    pub fn is_connected(&self) -> Memo<bool> {
        let state = self.state;
        Memo::new(move |_| state.get().is_connected())
    }

    pub fn is_connecting(&self) -> Memo<bool> {
        let state = self.state;
        Memo::new(move |_| matches!(state.get(), ConnectionState::Connecting))
    }

    pub fn is_closed(&self) -> Memo<bool> {
        let state = self.state;
        Memo::new(move |_| {
            matches!(
                state.get(),
                ConnectionState::Closed | ConnectionState::Disconnected
            )
        })
    }

    pub fn state(&self) -> ReadSignal<ConnectionState> {
        self.state
    }

    #[cfg(feature = "json")]
    pub fn message<T>(&self) -> Memo<Option<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'static,
    {
        let message = self.message;
        Memo::new(move |_| {
            message
                .get()
                .and_then(|raw| serde_json::from_str(&raw).ok())
        })
    }

    pub fn state_str(&self) -> Memo<&'static str> {
        let state = self.state;
        Memo::new(move |_| state.get().as_str())
    }

    pub fn raw_message(&self) -> ReadSignal<Option<String>> {
        self.message
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    pub fn send(&self, value: impl Into<String>) -> Result<(), NetError> {
        let msg = value.into();
        self.inner.with(|inner| {
            if let Some(socket) = &inner.socket {
                socket.send_with_str(&msg).map_err(NetError::from)
            } else {
                Err(NetError::ConnectionClosed(
                    "WebSocket is not connected".into(),
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
            .map_err(|err| NetError::SerializeError(err.to_string()))?;
        self.send(payload)
    }

    pub fn close(&self) -> Result<(), NetError> {
        let res = self.inner.with(|inner| {
            if let Some(socket) = &inner.socket {
                socket.close().map_err(NetError::from)
            } else {
                Ok(())
            }
        });
        self.inner.with(|inner| {
            inner.set_state.set(ConnectionState::Closed);
        });
        res
    }

    pub fn reconnect(&self) {
        if matches!(
            self.state.get(),
            ConnectionState::Closed | ConnectionState::Disconnected | ConnectionState::Error
        ) {
            self.inner.update(|inner| {
                inner.set_state.set(ConnectionState::Connecting);
                let socket_res = if inner.protocols.is_empty() {
                    JsWebSocket::new(&inner.url)
                } else {
                    let protocols = js_sys::Array::new();
                    for protocol in &inner.protocols {
                        protocols.push(&wasm_bindgen::JsValue::from_str(protocol));
                    }
                    JsWebSocket::new_with_str_sequence(&inner.url, &protocols.into())
                };

                if let Ok(new_socket) = socket_res {
                    new_socket
                        .set_onopen(Some(inner._on_open.as_ref().unchecked_ref::<Function>()));
                    new_socket.set_onmessage(Some(
                        inner._on_message.as_ref().unchecked_ref::<Function>(),
                    ));
                    new_socket
                        .set_onerror(Some(inner._on_error.as_ref().unchecked_ref::<Function>()));
                    new_socket
                        .set_onclose(Some(inner._on_close.as_ref().unchecked_ref::<Function>()));
                    inner.socket = Some(new_socket);
                }
            });
        }
    }

    pub fn toggle(&self) {
        if self.state.get().is_connected()
            || matches!(self.state.get(), ConnectionState::Connecting)
        {
            let _ = self.close();
        } else {
            self.reconnect();
        }
    }
}

#[derive(Clone)]
pub struct WebSocketBuilder {
    pub(crate) url: String,
    pub(crate) protocols: Vec<String>,
    pub(crate) auto_connect: bool,
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
        let (state, set_state) = Signal::pair(if self.auto_connect {
            ConnectionState::Connecting
        } else {
            ConnectionState::Disconnected
        });
        let (message, set_message) = Signal::pair(None::<String>);
        let (error, set_error) = Signal::pair(None::<String>);

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

        let socket = if self.auto_connect {
            let socket_obj = if self.protocols.is_empty() {
                JsWebSocket::new(&self.url).expect("failed to create WebSocket")
            } else {
                let protocols = js_sys::Array::new();
                for protocol in &self.protocols {
                    protocols.push(&wasm_bindgen::JsValue::from_str(protocol));
                }
                JsWebSocket::new_with_str_sequence(&self.url, &protocols.into())
                    .expect("failed to create WebSocket")
            };

            socket_obj.set_onopen(Some(on_open.as_ref().unchecked_ref::<Function>()));
            socket_obj.set_onmessage(Some(on_message.as_ref().unchecked_ref::<Function>()));
            socket_obj.set_onerror(Some(on_error.as_ref().unchecked_ref::<Function>()));
            socket_obj.set_onclose(Some(on_close.as_ref().unchecked_ref::<Function>()));
            Some(socket_obj)
        } else {
            None
        };

        WebSocketConnection {
            inner: StoredValue::new(WebSocketInner {
                socket,
                url: self.url,
                protocols: self.protocols,
                set_state,
                _on_open: on_open,
                _on_message: on_message,
                _on_error: on_error,
                _on_close: on_close,
            }),
            state,
            message,
            error,
        }
    }
}
