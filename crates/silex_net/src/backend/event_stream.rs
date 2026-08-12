use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource as JsEventSource, MessageEvent};

use silex_core::{
    CallbackInvokeError, CompletionSender, ErrorReporter, Memo, ReadSignal, RuntimeInputs,
    RwSignal, Scope, SilexError, SilexResult, StoredValue, WriteSignal, unwind_safe,
};

use crate::{
    NetError,
    builder::{IntoNetValue, ValueResolver},
    state::{ConnectionState, EventMessage},
};

/// Scope-owned EventSource connections.
///
/// EventSource keeps its browser-native reconnect behavior after a transport
/// error. This type does not add a second retry queue; [`EventStreamConnection::reconnect`]
/// performs an explicit source replacement when the caller wants to restart it.
pub struct EventStream;

impl EventStream {
    pub fn builder<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> EventStreamBuilder<'scope> {
        EventStreamBuilder::new(scope, url.into_net_value(), error_handler)
    }

    pub fn open<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<EventStreamConnection<'scope>, NetError> {
        Self::builder(scope, url, error_handler).try_build()
    }

    pub fn lazy<'scope>(
        scope: Scope<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> EventStreamBuilder<'scope> {
        Self::builder(scope, url, error_handler).auto_connect(false)
    }
}

#[derive(Copy, Clone)]
pub struct EventStreamConnection<'scope> {
    scope: Scope<'scope>,
    inner: StoredValue<'scope, EventStreamInner<'scope>>,
    state: ReadSignal<'scope, ConnectionState>,
    messages: RwSignal<'scope, Vec<EventMessage>>,
    error: ReadSignal<'scope, Option<NetError>>,
}

#[derive(Clone)]
enum EventStreamEvent {
    Open {
        generation: u64,
    },
    Message {
        generation: u64,
        event: Option<String>,
        data: String,
    },
    Error {
        generation: u64,
        error: NetError,
    },
}

struct HostRegistration {
    source: JsEventSource,
    event_name: Option<String>,
    gate: Rc<Cell<bool>>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(Event)>,
}

fn create_source(url: &str) -> Result<JsEventSource, NetError> {
    JsEventSource::new(url).map_err(NetError::from)
}

fn submit_completion<T: 'static>(
    token: &CompletionSender<T>,
    value: T,
    error_handler: ErrorReporter<'static>,
    gate: Option<&Cell<bool>>,
) {
    let result = token.submit(value);
    let Err(error) = result else {
        return;
    };
    let error = match error {
        CallbackInvokeError::Runtime(error) => SilexError::Reactivity(error),
        CallbackInvokeError::User(error) => error,
    };
    let handler_result = catch_unwind(AssertUnwindSafe(|| error_handler.handle(error)));
    if let Err(handler_panic) = handler_result {
        if let Some(gate) = gate {
            gate.set(false);
        }
        let _ = catch_unwind(AssertUnwindSafe(|| token.cancel()));
        resume_unwind(handler_panic);
    }
}

fn erase_error_handler<'scope>(handler: ErrorReporter<'scope>) -> ErrorReporter<'static> {
    // SAFETY: the CompletionSender rejects stale submissions before this
    // owner-bound handler is accessed after scope disposal.
    unsafe { std::mem::transmute(handler) }
}

impl HostRegistration {
    fn new(
        source: JsEventSource,
        event_name: Option<String>,
        generation: u64,
        token: &CompletionSender<EventStreamEvent>,
        error_handler: ErrorReporter<'static>,
    ) -> Result<Self, NetError> {
        let gate = Rc::new(Cell::new(true));

        let open_gate = gate.clone();
        let open_token = token.clone();
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            if open_gate.get() {
                submit_completion(
                    &open_token,
                    EventStreamEvent::Open { generation },
                    error_handler,
                    Some(&open_gate),
                );
            }
        }) as Box<dyn FnMut(Event)>);

        let message_gate = gate.clone();
        let message_token = token.clone();
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            if message_gate.get() {
                submit_completion(
                    &message_token,
                    EventStreamEvent::Message {
                        generation,
                        event: Some(event.type_()),
                        data: event.data().as_string().unwrap_or_default(),
                    },
                    error_handler,
                    Some(&message_gate),
                );
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        let error_gate = gate.clone();
        let error_token = token.clone();
        let on_error = Closure::wrap(Box::new(move |_event: Event| {
            if error_gate.get() {
                submit_completion(
                    &error_token,
                    EventStreamEvent::Error {
                        generation,
                        error: NetError::TransportUnavailable,
                    },
                    error_handler,
                    Some(&error_gate),
                );
            }
        }) as Box<dyn FnMut(Event)>);

        source.set_onopen(Some(on_open.as_ref().unchecked_ref::<Function>()));
        if let Some(name) = &event_name {
            if let Err(error) = source.add_event_listener_with_callback(
                name,
                on_message.as_ref().unchecked_ref::<Function>(),
            ) {
                gate.set(false);
                source.set_onopen(None);
                source.set_onerror(None);
                source.close();
                return Err(NetError::from(error));
            }
        } else {
            source.set_onmessage(Some(on_message.as_ref().unchecked_ref::<Function>()));
        }
        source.set_onerror(Some(on_error.as_ref().unchecked_ref::<Function>()));

        Ok(Self {
            source,
            event_name,
            gate,
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
        })
    }
}

impl Drop for HostRegistration {
    fn drop(&mut self) {
        self.gate.set(false);
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

type DeferredCallback<'scope> = Box<dyn FnOnce() + 'scope>;

struct EventStreamInner<'scope> {
    url: ValueResolver<'scope>,
    event_name: Option<String>,
    max_messages: Option<usize>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(NetError) + 'scope>>,
    set_state: RwSignal<'scope, ConnectionState>,
    messages: RwSignal<'scope, Vec<EventMessage>>,
    set_error: WriteSignal<'scope, Option<NetError>>,
    error_handler: ErrorReporter<'scope>,
    completion: CompletionSender<EventStreamEvent>,
    registration: Option<HostRegistration>,
    generation: u64,
}

fn cleanup_stored_inner<'scope>(inner: StoredValue<'scope, EventStreamInner<'scope>>) {
    inner
        .try_update(EventStreamInner::cleanup)
        .unwrap_or_else(|error| panic!("EventStream cleanup failed: {error}"));
}

impl Drop for EventStreamInner<'_> {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl<'scope> EventStreamInner<'scope> {
    fn cleanup(&mut self) {
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        self.completion.cancel();
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

    fn try_open_current(&mut self) -> (Result<(), NetError>, Option<DeferredCallback<'scope>>) {
        self.try_open_current_with_source(None)
    }

    fn try_open_current_with_source(
        &mut self,
        source: Option<JsEventSource>,
    ) -> (Result<(), NetError>, Option<DeferredCallback<'scope>>) {
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.set_state.set(ConnectionState::Connecting);

        let source = match source {
            Some(source) => Ok(source),
            None => create_source(&self.url.resolve()),
        };
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                self.set_state.set(ConnectionState::Error);
                self.set_error.set(Some(error.clone()));
                return (Err(error.clone()), Some(self.defer_error(error)));
            }
        };
        match HostRegistration::new(
            source,
            self.event_name.clone(),
            generation,
            &self.completion,
            erase_error_handler(self.error_handler),
        ) {
            Ok(registration) => {
                self.registration = Some(registration);
                (Ok(()), None)
            }
            Err(error) => {
                self.set_state.set(ConnectionState::Error);
                self.set_error.set(Some(error.clone()));
                (Err(error.clone()), Some(self.defer_error(error)))
            }
        }
    }

    fn handle_event(&mut self, event: EventStreamEvent) -> Option<DeferredCallback<'scope>> {
        let mut callback = None;
        match event {
            EventStreamEvent::Open { generation } if generation == self.generation => {
                self.set_state.set(ConnectionState::Connected);
                self.set_error.set(None);
                callback = Some(self.defer_open());
            }
            EventStreamEvent::Message {
                generation,
                event,
                data,
            } if generation == self.generation => {
                self.messages.update(|messages| {
                    messages.push(EventMessage { event, data });
                    if let Some(max_messages) = self.max_messages {
                        let excess = messages.len().saturating_sub(max_messages);
                        if excess > 0 {
                            messages.drain(..excess);
                        }
                    }
                });
                self.set_state.set(ConnectionState::Connected);
            }
            EventStreamEvent::Error { generation, error } if generation == self.generation => {
                self.set_state.set(ConnectionState::Error);
                self.set_error.set(Some(error.clone()));
                callback = Some(self.defer_error(error));
            }
            _ => {}
        }
        callback
    }

    fn close(&mut self) {
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        self.set_state.set(ConnectionState::Closed);
    }
}

impl<'scope> EventStreamConnection<'scope> {
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
    pub fn messages<T>(&self) -> Memo<'scope, Vec<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let messages = self.messages;
        self.scope.memo(move |_| {
            messages
                .get()
                .into_iter()
                .filter_map(|message| serde_json::from_str(&message.data).ok())
                .collect()
        })
    }

    #[cfg(feature = "json")]
    pub fn last_message<T>(&self) -> Memo<'scope, Option<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let messages = self.messages;
        self.scope.memo(move |_| {
            messages
                .get()
                .last()
                .and_then(|message| serde_json::from_str(&message.data).ok())
        })
    }

    #[cfg(feature = "json")]
    pub fn latest_messages<T>(&self, limit: usize) -> Memo<'scope, Vec<T>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let messages = self.messages;
        self.scope.memo(move |_| {
            messages
                .get()
                .iter()
                .rev()
                .take(limit)
                .filter_map(|message| serde_json::from_str(&message.data).ok())
                .collect()
        })
    }

    pub fn raw_messages(&self) -> ReadSignal<'scope, Vec<EventMessage>> {
        self.messages.read_signal()
    }

    /// Return the latest typed connection error, if one was reported.
    pub fn error(&self) -> ReadSignal<'scope, Option<NetError>> {
        self.error
    }

    pub fn clear_messages(&self) {
        self.messages.set(Vec::new());
    }

    pub fn close(&self) {
        self.inner.update(EventStreamInner::close);
    }

    pub fn try_reconnect(&self) -> Result<(), NetError> {
        if !matches!(
            self.state.get(),
            ConnectionState::Closed | ConnectionState::Disconnected | ConnectionState::Error
        ) {
            return Ok(());
        }
        let (result, callbacks) = self.inner.update(EventStreamInner::try_open_current);
        if let Some(callback) = callbacks {
            callback();
        }
        result
    }

    pub fn reconnect(&self) {
        let _ = self.try_reconnect();
    }

    pub fn toggle(&self) {
        if self.state.get().is_active() {
            self.close();
        } else {
            self.reconnect();
        }
    }
}

#[derive(Clone)]
pub struct EventStreamBuilder<'scope> {
    scope: Scope<'scope>,
    error_handler: ErrorReporter<'scope>,
    url: ValueResolver<'scope>,
    event_name: Option<String>,
    auto_connect: bool,
    max_messages: Option<usize>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(NetError) + 'scope>>,
}

impl<'scope> EventStreamBuilder<'scope> {
    fn new(
        scope: Scope<'scope>,
        url: ValueResolver<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> Self {
        Self {
            scope,
            error_handler,
            url,
            event_name: None,
            auto_connect: true,
            max_messages: None,
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

    pub fn max_messages(mut self, max_messages: usize) -> Self {
        self.max_messages = Some(max_messages);
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

    pub fn try_build(self) -> Result<EventStreamConnection<'scope>, NetError> {
        let Self {
            scope,
            error_handler,
            url,
            event_name,
            auto_connect,
            max_messages,
            on_open,
            on_error,
        } = self;
        let mut inputs = RuntimeInputs::new();
        inputs.extend(&url.inputs());
        scope
            .try_validate_inputs(&inputs)
            .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

        let initial_source = if auto_connect {
            match create_source(&url.resolve()) {
                Ok(source) => Some(source),
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

        let state = scope.rw_signal(if auto_connect {
            ConnectionState::Connecting
        } else {
            ConnectionState::Disconnected
        });
        let messages = scope.rw_signal(Vec::<EventMessage>::new());
        let (error, set_error) = scope.signal(None::<NetError>);
        let inner_slot = Rc::new(Cell::new(
            None::<StoredValue<'scope, EventStreamInner<'scope>>>,
        ));
        let inner_slot_for_completion = inner_slot.clone();
        let completion = scope.completion_sender(unwind_safe(move |event: EventStreamEvent| {
            if let Some(inner) = inner_slot_for_completion.get()
                && let Some(callback) = inner.update(|inner| inner.handle_event(event))
            {
                callback();
            }
            Ok(())
        }));
        let inner = scope.stored(EventStreamInner {
            url,
            event_name,
            max_messages,
            on_open,
            on_error,
            set_state: state,
            messages,
            set_error,
            error_handler,
            completion,
            registration: None,
            generation: 0,
        });
        inner_slot.set(Some(inner));
        scope
            .on_cleanup(
                move || -> SilexResult<()> {
                    cleanup_stored_inner(inner);
                    Ok(())
                },
                error_handler,
            )
            .map_err(|error| NetError::InvalidConfiguration(error.to_string()))?;

        let connection = EventStreamConnection {
            scope,
            inner,
            state: state.read_signal(),
            messages,
            error,
        };
        if auto_connect {
            let (result, callbacks) =
                inner.update(|inner| inner.try_open_current_with_source(initial_source));
            if let Some(callback) = callbacks {
                callback();
            }
            if let Err(error) = result {
                cleanup_stored_inner(inner);
                return Err(error);
            }
        }
        Ok(connection)
    }

    pub fn build(self) -> EventStreamConnection<'scope> {
        self.try_build()
            .unwrap_or_else(|error| panic!("创建 EventStream 失败: {error:?}"))
    }
}
