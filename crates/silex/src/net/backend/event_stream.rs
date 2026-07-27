use std::rc::Rc;

use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource as JsEventSource, MessageEvent};

use silex_core::{
    reactivity::{Memo, ReadSignal, RwSignal, Signal, StoredValue},
    traits::{RxGet, RxRead, RxWrite},
};

use crate::net::state::{ConnectionState, EventMessage};

pub struct EventStream;

impl EventStream {
    pub fn builder(url: impl Into<String>) -> EventStreamBuilder {
        EventStreamBuilder::new(url)
    }

    pub fn open(url: impl Into<String>) -> EventStreamConnection {
        Self::builder(url).build()
    }

    pub fn lazy(url: impl Into<String>) -> EventStreamConnection {
        Self::builder(url).auto_connect(false).build()
    }
}

#[derive(Copy, Clone)]
pub struct EventStreamConnection {
    inner: StoredValue<EventStreamInner>,
    state: RwSignal<ConnectionState>,
    messages: RwSignal<Vec<EventMessage>>,
    error: ReadSignal<Option<String>>,
}
struct EventStreamInner {
    source: Option<JsEventSource>,
    url: String,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(Event)>,
    event_name: Option<String>,
}

impl Drop for EventStreamInner {
    fn drop(&mut self) {
        if let Some(source) = &self.source {
            source.set_onopen(None);
            source.set_onerror(None);
            if let Some(name) = &self.event_name {
                let _ = source.remove_event_listener_with_callback(
                    name,
                    self._on_message.as_ref().unchecked_ref(),
                );
            } else {
                source.set_onmessage(None);
            }
            source.close();
        }
    }
}

impl EventStreamConnection {
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
        self.state.read_signal()
    }

    #[cfg(feature = "json")]
    pub fn messages<T>(&self) -> Memo<Vec<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'static,
    {
        let messages = self.messages;
        Memo::new(move |_| {
            messages
                .get()
                .into_iter()
                .filter_map(|msg| serde_json::from_str(&msg.data).ok())
                .collect()
        })
    }

    #[cfg(feature = "json")]
    pub fn last_message<T>(&self) -> Memo<Option<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'static,
    {
        let messages = self.messages;
        Memo::new(move |_| {
            messages
                .get()
                .last()
                .and_then(|msg| serde_json::from_str(&msg.data).ok())
        })
    }

    #[cfg(feature = "json")]
    pub fn latest_messages<T>(&self, limit: usize) -> Memo<Vec<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'static,
    {
        let messages = self.messages;
        Memo::new(move |_| {
            messages
                .get()
                .iter()
                .rev()
                .filter_map(|msg| serde_json::from_str(&msg.data).ok())
                .take(limit)
                .collect()
        })
    }

    pub fn raw_messages(&self) -> ReadSignal<Vec<EventMessage>> {
        self.messages.read_signal()
    }

    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    pub fn clear_messages(&self) {
        self.messages.set(Vec::new());
    }

    pub fn close(&self) {
        self.inner.with(|inner| {
            if let Some(source) = &inner.source {
                source.close();
            }
        });
        self.state.set(ConnectionState::Closed);
    }

    pub fn reconnect(&self) {
        if matches!(
            self.state.get(),
            ConnectionState::Closed | ConnectionState::Disconnected | ConnectionState::Error
        ) {
            self.state.set(ConnectionState::Connecting);
            self.inner.update(|inner| {
                if let Ok(new_source) = JsEventSource::new(&inner.url) {
                    new_source
                        .set_onopen(Some(inner._on_open.as_ref().unchecked_ref::<Function>()));
                    if let Some(event_name) = &inner.event_name {
                        let _ = new_source.add_event_listener_with_callback(
                            event_name,
                            inner._on_message.as_ref().unchecked_ref::<Function>(),
                        );
                    } else {
                        new_source.set_onmessage(Some(
                            inner._on_message.as_ref().unchecked_ref::<Function>(),
                        ));
                    }
                    new_source
                        .set_onerror(Some(inner._on_error.as_ref().unchecked_ref::<Function>()));
                    inner.source = Some(new_source);
                }
            });
        }
    }

    pub fn toggle(&self) {
        if self.state.get().is_connected()
            || matches!(self.state.get(), ConnectionState::Connecting)
        {
            self.close();
        } else {
            self.reconnect();
        }
    }
}

#[derive(Clone)]
pub struct EventStreamBuilder {
    pub(crate) url: String,
    pub(crate) event_name: Option<String>,
    pub(crate) auto_connect: bool,
    pub(crate) on_open: Vec<Rc<dyn Fn()>>,
    pub(crate) on_error: Vec<Rc<dyn Fn(String)>>,
}

impl EventStreamBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            event_name: None,
            auto_connect: true,
            on_open: Vec::new(),
            on_error: Vec::new(),
        }
    }

    pub fn auto_connect(mut self, auto_connect: bool) -> Self {
        self.auto_connect = auto_connect;
        self
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
        let state = RwSignal::new(if self.auto_connect {
            ConnectionState::Connecting
        } else {
            ConnectionState::Disconnected
        });
        let messages = RwSignal::new(Vec::<EventMessage>::new());
        let (error, set_error) = Signal::pair(None::<String>);

        let on_open_handlers = self.on_open.clone();
        let on_error_handlers = self.on_error.clone();

        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            state.set(ConnectionState::Connected);
            for handler in &on_open_handlers {
                handler();
            }
        }) as Box<dyn FnMut(Event)>);

        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data().as_string().unwrap_or_default();
            let event_name = event.type_();
            messages.update(|msgs: &mut Vec<EventMessage>| {
                msgs.push(EventMessage {
                    event: Some(event_name),
                    data,
                });
            });
            state.set(ConnectionState::Connected);
        }) as Box<dyn FnMut(MessageEvent)>);

        let on_error = Closure::wrap(Box::new(move |_event: Event| {
            state.set(ConnectionState::Error);
            set_error.set(Some("event stream error".to_string()));
            for handler in &on_error_handlers {
                handler("event stream error".to_string());
            }
        }) as Box<dyn FnMut(Event)>);

        let source = if self.auto_connect {
            let s = JsEventSource::new(&self.url).expect("failed to create EventSource");
            s.set_onopen(Some(on_open.as_ref().unchecked_ref::<Function>()));
            if let Some(event_name) = &self.event_name {
                let on_message_fn = on_message.as_ref().unchecked_ref::<Function>();
                s.add_event_listener_with_callback(event_name, on_message_fn)
                    .expect("failed to register event listener");
            } else {
                s.set_onmessage(Some(on_message.as_ref().unchecked_ref::<Function>()));
            }
            s.set_onerror(Some(on_error.as_ref().unchecked_ref::<Function>()));
            Some(s)
        } else {
            None
        };

        EventStreamConnection {
            inner: StoredValue::new(EventStreamInner {
                source,
                url: self.url,
                _on_open: on_open,
                _on_message: on_message,
                _on_error: on_error,
                event_name: self.event_name,
            }),
            state,
            messages,
            error,
        }
    }
}
