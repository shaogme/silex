use std::rc::Rc;

use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource as JsEventSource, MessageEvent};

use silex_core::{
    reactivity::{Memo, ReadSignal, Signal},
    traits::{RxGet, RxWrite},
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
    pub fn messages<T>(&self) -> Memo<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Clone + PartialEq + 'static,
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
        T: serde::de::DeserializeOwned + Clone + PartialEq + 'static,
    {
        let messages = self.messages;
        Memo::new(move |_| {
            messages
                .get()
                .last()
                .and_then(|msg| serde_json::from_str(&msg.data).ok())
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

        let (state, set_state) = Signal::pair(ConnectionState::Connecting);
        let (messages, set_messages) = Signal::pair(Vec::<EventMessage>::new());
        let (error, set_error) = Signal::pair(None::<String>);

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
                .add_event_listener_with_callback(event_name, on_message_fn)
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
