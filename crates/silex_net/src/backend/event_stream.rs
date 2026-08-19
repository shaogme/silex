use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use js_sys::Function;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Event, EventSource as JsEventSource, MessageEvent};

use silex_core::{
    CallbackInvokeError, CompletionSender, ErrorHandlerInput, ErrorReporter, OwnerAccess,
    ReactiveError, ReadSignal, RwSignal, Rx, SilexError, SilexErrorKind, SilexResult, StoredValue,
    WriteSignal, unwind_safe,
};

use crate::{
    NetError, NetErrorKind,
    builder::{IntoNetValue, ValueResolver},
    state::{ConnectionState, EventMessage},
};

/// Owner-owned EventSource connections.
///
/// EventSource keeps its browser-native reconnect behavior after a transport
/// error. This type does not add a second retry queue; [`EventStreamConnection::reconnect`]
/// performs an explicit source replacement when the caller wants to restart it.
pub struct EventStream;

impl EventStream {
    pub fn builder<'scope, H>(
        scope: OwnerAccess<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: H,
    ) -> EventStreamBuilder<'scope, H>
    where
        H: ErrorHandlerInput<'scope>,
    {
        EventStreamBuilder::new(scope, url.into_net_value(), error_handler)
    }

    pub fn open<'scope, H>(
        scope: OwnerAccess<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: H,
    ) -> Result<EventStreamConnection<'scope>, NetError>
    where
        H: ErrorHandlerInput<'scope>,
    {
        Self::builder(scope, url, error_handler).build()
    }

    pub fn lazy<'scope, H>(
        scope: OwnerAccess<'scope>,
        url: impl IntoNetValue<'scope>,
        error_handler: H,
    ) -> EventStreamBuilder<'scope, H>
    where
        H: ErrorHandlerInput<'scope>,
    {
        Self::builder(scope, url, error_handler).auto_connect(false)
    }
}

#[derive(Copy, Clone)]
pub struct EventStreamConnection<'scope> {
    scope: OwnerAccess<'scope>,
    inner: StoredValue<'scope, EventStreamInner<'scope>>,
    state: ReadSignal<'scope, ConnectionState>,
    messages: RwSignal<'scope, Vec<EventMessage>>,
    error: ReadSignal<'scope, Option<NetError>>,
    error_handler: ErrorReporter<'scope>,
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
        source: JsEventSource,
        event_name: Option<String>,
        generation: u64,
        token: &CompletionSender<EventStreamEvent>,
        error_token: &CompletionSender<SilexError>,
    ) -> Result<Self, NetError> {
        let gate = Rc::new(Cell::new(true));

        let open_gate = gate.clone();
        let open_token = token.clone();
        let open_error_token = error_token.clone();
        let on_open = Closure::wrap_assert_unwind_safe(Box::new(move |_event: Event| {
            if open_gate.get() {
                submit_completion(
                    &open_token,
                    &open_error_token,
                    EventStreamEvent::Open { generation },
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
                        EventStreamEvent::Error {
                            generation,
                            error: NetError::recoverable(NetErrorKind::JsError(
                                "EventSource message data is not a string".to_string(),
                            )),
                        },
                        Some(&message_gate),
                    );
                    return;
                };
                submit_completion(
                    &message_token,
                    &message_error_token,
                    EventStreamEvent::Message {
                        generation,
                        event: Some(event.type_()),
                        data,
                    },
                    Some(&message_gate),
                );
            }
        })
            as Box<dyn FnMut(MessageEvent)>);

        let error_gate = gate.clone();
        let event_error_token = token.clone();
        let event_error_completion_token = error_token.clone();
        let on_error = Closure::wrap_assert_unwind_safe(Box::new(move |_event: Event| {
            if error_gate.get() {
                submit_completion(
                    &event_error_token,
                    &event_error_completion_token,
                    EventStreamEvent::Error {
                        generation,
                        error: NetError::recoverable(NetErrorKind::TransportUnavailable),
                    },
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
    completion: CompletionSender<EventStreamEvent>,
    error_completion: CompletionSender<SilexError>,
    registration: Option<HostRegistration>,
    generation: u64,
}

fn cleanup_stored_inner<'scope>(
    inner: StoredValue<'scope, EventStreamInner<'scope>>,
) -> SilexResult<()> {
    inner.update(EventStreamInner::cleanup)?
}

impl Drop for EventStreamInner<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl<'scope> EventStreamInner<'scope> {
    fn cleanup(&mut self) -> SilexResult<()> {
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        self.completion
            .cancel()
            .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))?;
        Ok(())
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

    fn try_open_current(
        &mut self,
    ) -> SilexResult<(Result<(), NetError>, Option<DeferredCallback<'scope>>)> {
        self.try_open_current_with_source(None)
    }

    fn try_open_current_with_source(
        &mut self,
        source: Option<JsEventSource>,
    ) -> SilexResult<(Result<(), NetError>, Option<DeferredCallback<'scope>>)> {
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.set_state.set(ConnectionState::Connecting)?;

        let source = match source {
            Some(source) => Ok(source),
            None => self
                .url
                .resolve()
                .map_err(NetError::from)
                .and_then(|url| create_source(&url)),
        };
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                self.set_state.set(ConnectionState::Error)?;
                self.set_error.set(Some(error.clone()))?;
                return Ok((Err(error.clone()), Some(self.defer_error(error))));
            }
        };
        match HostRegistration::new(
            source,
            self.event_name.clone(),
            generation,
            &self.completion,
            &self.error_completion,
        ) {
            Ok(registration) => {
                self.registration = Some(registration);
                Ok((Ok(()), None))
            }
            Err(error) => {
                self.set_state.set(ConnectionState::Error)?;
                self.set_error.set(Some(error.clone()))?;
                Ok((Err(error.clone()), Some(self.defer_error(error))))
            }
        }
    }

    fn handle_event(
        &mut self,
        event: EventStreamEvent,
    ) -> SilexResult<Option<DeferredCallback<'scope>>> {
        let mut callback = None;
        match event {
            EventStreamEvent::Open { generation } if generation == self.generation => {
                self.set_state.set(ConnectionState::Connected)?;
                self.set_error.set(None)?;
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
                })?;
                self.set_state.set(ConnectionState::Connected)?;
            }
            EventStreamEvent::Error { generation, error } if generation == self.generation => {
                self.set_state.set(ConnectionState::Error)?;
                self.set_error.set(Some(error.clone()))?;
                callback = Some(self.defer_error(error));
            }
            _ => {}
        }
        Ok(callback)
    }

    fn close(&mut self) -> SilexResult<()> {
        self.registration.take();
        self.generation = self.generation.wrapping_add(1);
        self.set_state.set(ConnectionState::Closed)?;
        Ok(())
    }
}

impl<'scope> EventStreamConnection<'scope> {
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
    pub fn messages<T>(&self) -> SilexResult<Rx<'scope, Vec<T>>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let messages = self.messages;
        let handler = self.error_handler;
        self.scope
            .computed(
                move || {
                    messages
                        .get()?
                        .into_iter()
                        .map(|message| {
                            serde_json::from_str(&message.data).map_err(|error| {
                                NetError::recoverable(NetErrorKind::DecodeError(format!(
                                    "decode EventStream message failed: {error}"
                                )))
                                .into()
                            })
                        })
                        .collect()
                },
                handler,
            )
            .map(|memo| memo.into_rx())
    }

    #[cfg(feature = "json")]
    pub fn last_message<T>(&self) -> SilexResult<Rx<'scope, Option<T>>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let messages = self.messages;
        let handler = self.error_handler;
        self.scope
            .computed(
                move || {
                    messages
                        .get()?
                        .last()
                        .map(|message| {
                            serde_json::from_str(&message.data).map_err(|error| {
                                NetError::recoverable(NetErrorKind::DecodeError(format!(
                                    "decode EventStream message failed: {error}"
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

    #[cfg(feature = "json")]
    pub fn latest_messages<T>(&self, limit: usize) -> SilexResult<Rx<'scope, Vec<T>>>
    where
        T: serde::de::DeserializeOwned + PartialEq + 'scope,
    {
        let messages = self.messages;
        let handler = self.error_handler;
        self.scope
            .computed(
                move || {
                    messages
                        .get()?
                        .iter()
                        .rev()
                        .take(limit)
                        .map(|message| {
                            serde_json::from_str(&message.data).map_err(|error| {
                                NetError::recoverable(NetErrorKind::DecodeError(format!(
                                    "decode EventStream message failed: {error}"
                                )))
                                .into()
                            })
                        })
                        .collect()
                },
                handler,
            )
            .map(|memo| memo.into_rx())
    }

    pub fn raw_messages(&self) -> ReadSignal<'scope, Vec<EventMessage>> {
        self.messages.read_signal()
    }

    /// Return the latest typed connection error, if one was reported.
    pub fn error(&self) -> ReadSignal<'scope, Option<NetError>> {
        self.error
    }

    pub fn clear_messages(&self) -> SilexResult<()> {
        self.messages.set(Vec::new())
    }

    pub fn close(&self) -> SilexResult<()> {
        let _ = self.inner.update(EventStreamInner::close)?;
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
            .update(EventStreamInner::try_open_current)
            .map_err(NetError::from)??;
        if let Some(callback) = callbacks {
            callback();
        }
        result
    }

    pub fn toggle(&self) -> Result<(), NetError> {
        if self.state.get().map_err(NetError::from)?.is_active() {
            self.close().map_err(NetError::from)?;
        } else {
            self.reconnect()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct EventStreamBuilder<'scope, H = ErrorReporter<'scope>> {
    scope: OwnerAccess<'scope>,
    error_handler: H,
    url: ValueResolver<'scope>,
    event_name: Option<String>,
    auto_connect: bool,
    max_messages: Option<usize>,
    on_open: Vec<Rc<dyn Fn() + 'scope>>,
    on_error: Vec<Rc<dyn Fn(NetError) + 'scope>>,
}

impl<'scope, H> EventStreamBuilder<'scope, H> {
    fn new(scope: OwnerAccess<'scope>, url: ValueResolver<'scope>, error_handler: H) -> Self {
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

    pub fn build(self) -> Result<EventStreamConnection<'scope>, NetError>
    where
        H: ErrorHandlerInput<'scope>,
    {
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
        let error_handler = error_handler.handler_ref();
        url.validate_runtime(scope).map_err(NetError::from)?;
        let initial_source = if auto_connect {
            match url
                .resolve()
                .map_err(NetError::from)
                .and_then(|url| create_source(&url))
            {
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
        })?;
        let messages = scope.rw_signal(Vec::<EventMessage>::new())?;
        let (error, set_error) = scope.signal(None::<NetError>)?;
        let inner_slot = Rc::new(Cell::new(
            None::<StoredValue<'scope, EventStreamInner<'scope>>>,
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
        let completion = scope.completion_sender(unwind_safe(move |event: EventStreamEvent| {
            if let Some(inner) = inner_slot_for_completion.get() {
                let callback = inner.update(|inner| inner.handle_event(event))??;
                if let Some(callback) = callback {
                    callback();
                }
            }
            Ok(())
        }))?;
        let inner = scope.stored(EventStreamInner {
            url,
            event_name,
            max_messages,
            on_open,
            on_error,
            set_state: state,
            messages,
            set_error,
            completion,
            error_completion,
            registration: None,
            generation: 0,
        })?;
        inner_slot.set(Some(inner));
        scope
            .on_cleanup(
                move || -> SilexResult<()> { cleanup_stored_inner(inner) },
                error_handler,
            )
            .map_err(NetError::from)?;

        let connection = EventStreamConnection {
            scope,
            inner,
            state: state.read_signal(),
            messages,
            error,
            error_handler,
        };
        if auto_connect {
            let (result, callbacks) = inner
                .update(|inner| inner.try_open_current_with_source(initial_source))
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
