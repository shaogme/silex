use crate::{PersistenceError, PersistenceErrorKind};
use js_sys::Object;
use ref_str::LocalStaticRefStr;
use silex_core::{ErrorReporter, OwnerAccess, Rx, SilexResult};
use silex_router::{Navigator, RouterContext};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Storage, StorageEvent};

/// A backend-originated change delivered to persistent bindings.
#[derive(Clone, Debug, PartialEq)]
pub enum BackendEvent {
    Set {
        key: LocalStaticRefStr,
        value: String,
    },
    Removed {
        key: LocalStaticRefStr,
    },
    ExternalRefresh,
}

/// Static host bridge used by storage and query event sources.
pub type BackendEventSink = Rc<dyn Fn(BackendEvent) + 'static>;
type BackendErrorSink = Rc<dyn Fn(PersistenceError) + 'static>;
type BackendErrorSinkSlot = Rc<RefCell<Option<BackendErrorSink>>>;
type ListenerRemoval = (Closure<dyn FnMut(StorageEvent)>, Vec<BackendErrorSinkSlot>);

pub struct BackendSubscription<'scope> {
    cleanup: Option<Box<dyn FnOnce() + 'scope>>,
    error_sink: Option<BackendErrorSinkSlot>,
}

impl<'scope> BackendSubscription<'scope> {
    pub fn new(cleanup: impl FnOnce() + 'scope) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
            error_sink: None,
        }
    }

    fn with_error_sink_slot(
        cleanup: impl FnOnce() + 'scope,
        error_sink: BackendErrorSinkSlot,
    ) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
            error_sink: Some(error_sink),
        }
    }

    pub(crate) fn set_error_sink(&mut self, sink: BackendErrorSink) {
        if let Some(error_sink) = &self.error_sink {
            *error_sink.borrow_mut() = Some(sink);
        }
    }

    pub fn cleanup(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

impl Drop for BackendSubscription<'_> {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Error returned when a backend cannot finish creating a subscription.
///
/// The cleanup token is mandatory even when no host resource was created. A
/// backend must move every resource created before the error into this token;
/// the builder consumes it before interpreting the error.
pub struct BackendSubscribeError<'scope> {
    error: PersistenceError,
    cleanup: BackendSubscription<'scope>,
}

impl fmt::Debug for BackendSubscribeError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendSubscribeError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<'scope> BackendSubscribeError<'scope> {
    pub fn new(error: PersistenceError) -> Self {
        Self {
            error,
            cleanup: BackendSubscription::new(|| {}),
        }
    }

    pub fn with_cleanup(error: PersistenceError, cleanup: BackendSubscription<'scope>) -> Self {
        Self { error, cleanup }
    }

    pub fn into_error(mut self) -> PersistenceError {
        self.cleanup.cleanup();
        self.error
    }
}

impl<'scope> From<PersistenceError> for BackendSubscribeError<'scope> {
    fn from(error: PersistenceError) -> Self {
        Self::new(error)
    }
}

impl<'scope> BackendSubscription<'scope> {
    pub fn into_error(self, error: PersistenceError) -> BackendSubscribeError<'scope> {
        BackendSubscribeError::with_cleanup(error, self)
    }
}

pub trait PersistenceBackend<'scope>: Clone + 'scope {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError>;
    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError>;
    fn remove(&self, key: &str) -> Result<(), PersistenceError>;

    fn subscribe(
        &self,
        owner: OwnerAccess<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>>;
}

/// Synchronous Web Storage backend for `localStorage` or `sessionStorage`.
///
/// Values are stored as readable plaintext in the browser profile. Do not use
/// this backend for passwords, tokens, or other long-lived credentials.
#[derive(Clone, Debug)]
pub struct WebStorageBackend<const IS_LOCAL: bool> {
    storage: Option<Storage>,
}

impl<const IS_LOCAL: bool> WebStorageBackend<IS_LOCAL> {
    pub fn new() -> Self {
        Self {
            storage: storage_handle(Self::kind()).ok(),
        }
    }

    fn kind() -> StorageAreaKind {
        if IS_LOCAL {
            StorageAreaKind::Local
        } else {
            StorageAreaKind::Session
        }
    }

    fn storage(&self) -> Result<&Storage, PersistenceError> {
        self.storage.as_ref().ok_or(PersistenceError::recoverable(
            PersistenceErrorKind::BackendUnavailable,
        ))
    }
}

impl<const IS_LOCAL: bool> Default for WebStorageBackend<IS_LOCAL> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const IS_LOCAL: bool> PartialEq for WebStorageBackend<IS_LOCAL> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<const IS_LOCAL: bool> Eq for WebStorageBackend<IS_LOCAL> {}

pub type LocalStorageBackend = WebStorageBackend<true>;
pub type SessionStorageBackend = WebStorageBackend<false>;

#[derive(Clone)]
pub struct QueryBackend<'scope> {
    navigator: Option<Navigator<'scope>>,
    query_map: Option<Rx<'scope, HashMap<String, String>>>,
}

impl<'scope> QueryBackend<'scope> {
    pub fn new(ctx: RouterContext<'scope>) -> Self {
        let navigator = ctx.navigator;
        let query_map = ctx.query_map();

        Self {
            navigator: Some(navigator),
            query_map: Some(query_map),
        }
    }

    pub fn unavailable() -> Self {
        Self {
            navigator: None,
            query_map: None,
        }
    }

    fn navigator(&self) -> Result<&Navigator<'scope>, PersistenceError> {
        self.navigator.as_ref().ok_or(PersistenceError::recoverable(
            PersistenceErrorKind::BackendUnavailable,
        ))
    }

    fn query_map(&self) -> Result<Rx<'scope, HashMap<String, String>>, PersistenceError> {
        self.query_map.ok_or(PersistenceError::recoverable(
            PersistenceErrorKind::BackendUnavailable,
        ))
    }
}

impl<'scope, const IS_LOCAL: bool> PersistenceBackend<'scope> for WebStorageBackend<IS_LOCAL> {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        storage_get(self.storage()?, key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        storage_set(self.storage()?, key, value)
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        storage_remove(self.storage()?, key)
    }

    fn subscribe(
        &self,
        _owner: OwnerAccess<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        _error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        subscribe_storage(Self::kind(), key.into(), sink)
    }
}

impl<'scope> PersistenceBackend<'scope> for QueryBackend<'scope> {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        Ok(self
            .query_map()?
            .get_untracked()
            .map_err(PersistenceError::from)?
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        self.navigator()?
            .set_query(key, Some(value))
            .map_err(PersistenceError::from)?;
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        self.navigator()?
            .set_query(key, None)
            .map_err(PersistenceError::from)?;
        Ok(())
    }

    fn subscribe(
        &self,
        owner: OwnerAccess<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        let key = key.into();
        let query_map = self.query_map().map_err(BackendSubscribeError::new)?;
        owner.validate_runtime(&query_map).map_err(|error| {
            BackendSubscribeError::new(PersistenceError::fatal(
                PersistenceErrorKind::InvalidConfiguration(error.to_string()),
            ))
        })?;
        let active = Rc::new(Cell::new(true));
        let active_for_effect = active.clone();
        let key_for_effect = key.clone();
        owner
            .effect_with_previous(
                move |previous: Option<&Option<String>>| -> SilexResult<Option<String>> {
                    let current = query_map.get()?.get(key_for_effect.as_ref()).cloned();
                    if active_for_effect.get()
                        && let Some(previous) = previous
                        && previous != &current
                    {
                        match current.clone() {
                            Some(value) => sink(BackendEvent::Set {
                                key: key_for_effect.clone(),
                                value,
                            }),
                            None => sink(BackendEvent::Removed {
                                key: key_for_effect.clone(),
                            }),
                        }
                    }
                    Ok(current)
                },
                error_handler,
            )
            .map_err(|error| {
                BackendSubscribeError::new(PersistenceError::fatal(
                    PersistenceErrorKind::InvalidConfiguration(error.to_string()),
                ))
            })?;

        Ok(BackendSubscription::new(move || {
            active.set(false);
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StorageAreaKind {
    Local,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListenerState {
    Detached,
    Attached,
    DetachQueued(u64),
    Removing(u64),
}

#[derive(Debug)]
struct StorageHubState {
    subscriber_count: usize,
    generation: u64,
    listener: ListenerState,
}

impl Default for StorageHubState {
    fn default() -> Self {
        Self {
            subscriber_count: 0,
            generation: 0,
            listener: ListenerState::Detached,
        }
    }
}

impl StorageHubState {
    fn subscribe(&mut self) {
        self.subscriber_count += 1;
        self.generation = self.generation.wrapping_add(1);
        if matches!(
            self.listener,
            ListenerState::DetachQueued(_) | ListenerState::Detached
        ) {
            self.listener = ListenerState::Attached;
        }
    }

    fn unsubscribe(&mut self) -> Option<u64> {
        debug_assert!(self.subscriber_count > 0);
        self.subscriber_count = self.subscriber_count.saturating_sub(1);
        self.generation = self.generation.wrapping_add(1);
        if self.subscriber_count == 0 && matches!(self.listener, ListenerState::Attached) {
            let generation = self.generation;
            self.listener = ListenerState::DetachQueued(generation);
            Some(generation)
        } else {
            None
        }
    }

    fn begin_remove(&mut self, generation: u64) -> bool {
        if self.subscriber_count == 0
            && matches!(self.listener, ListenerState::DetachQueued(current) if current == generation)
        {
            self.listener = ListenerState::Removing(generation);
            true
        } else {
            false
        }
    }

    fn finish_remove(&mut self, generation: u64, removed: bool) -> bool {
        if !matches!(self.listener, ListenerState::Removing(current) if current == generation) {
            return false;
        }
        if removed {
            self.listener = ListenerState::Detached;
            self.subscriber_count > 0
        } else {
            self.listener = ListenerState::Attached;
            false
        }
    }

    fn mark_attached(&mut self) {
        self.listener = ListenerState::Attached;
    }

    fn listener(&self) -> ListenerState {
        self.listener
    }
}

struct StorageSubscriber {
    id: usize,
    sink: BackendEventSink,
    error_sink: BackendErrorSinkSlot,
}

struct StorageHub {
    state: StorageHubState,
    subscribers: HashMap<(StorageAreaKind, LocalStaticRefStr), Vec<StorageSubscriber>>,
    next_id: usize,
    closure: Option<Closure<dyn FnMut(StorageEvent)>>,
    local_storage: Option<Storage>,
    session_storage: Option<Storage>,
    detach_error_sinks: Vec<BackendErrorSinkSlot>,
}

impl Default for StorageHub {
    fn default() -> Self {
        Self {
            state: StorageHubState::default(),
            subscribers: HashMap::new(),
            next_id: 0,
            closure: None,
            local_storage: web_sys::window()
                .and_then(|window| window.local_storage().ok().flatten()),
            session_storage: web_sys::window()
                .and_then(|window| window.session_storage().ok().flatten()),
            detach_error_sinks: Vec::new(),
        }
    }
}

thread_local! {
    static STORAGE_HUB: RefCell<StorageHub> = RefCell::new(StorageHub::default());
}

impl StorageHub {
    fn subscribe(
        &mut self,
        kind: StorageAreaKind,
        key: LocalStaticRefStr,
        sink: BackendEventSink,
        error_sink: BackendErrorSinkSlot,
    ) -> Result<usize, PersistenceError> {
        if !matches!(self.state.listener(), ListenerState::Removing(_)) {
            self.ensure_listener()?;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.state.subscribe();
        self.subscribers
            .entry((kind, key))
            .or_default()
            .push(StorageSubscriber {
                id,
                sink,
                error_sink,
            });
        Ok(id)
    }

    fn unsubscribe(
        &mut self,
        kind: StorageAreaKind,
        key: impl Into<LocalStaticRefStr>,
        id: usize,
        error_sink: BackendErrorSinkSlot,
    ) -> Option<u64> {
        let key = key.into();
        let mut removed = false;
        if let Some(subscribers) = self.subscribers.get_mut(&(kind, key.clone())) {
            let before = subscribers.len();
            subscribers.retain(|subscriber| subscriber.id != id);
            removed = before != subscribers.len();
            if subscribers.is_empty() {
                self.subscribers.remove(&(kind, key));
            }
        }

        if removed {
            self.detach_error_sinks.push(error_sink);
            self.state.unsubscribe()
        } else {
            None
        }
    }

    fn begin_remove(&mut self, generation: u64) -> Option<ListenerRemoval> {
        if !self.state.begin_remove(generation) {
            return None;
        }
        let closure = self.closure.take()?;
        let error_sinks = std::mem::take(&mut self.detach_error_sinks);
        Some((closure, error_sinks))
    }

    fn finish_remove(
        &mut self,
        generation: u64,
        closure: Closure<dyn FnMut(StorageEvent)>,
        removed: bool,
        error_sinks: Vec<BackendErrorSinkSlot>,
    ) -> (bool, Vec<BackendErrorSinkSlot>) {
        let attach_listener = self.state.finish_remove(generation, removed);
        if !removed {
            self.closure = Some(closure);
        }
        (attach_listener, error_sinks)
    }

    fn error_sinks_for_subscribers(&self) -> Vec<BackendErrorSinkSlot> {
        self.subscribers
            .values()
            .flat_map(|subscribers| {
                subscribers
                    .iter()
                    .map(|subscriber| subscriber.error_sink.clone())
            })
            .collect()
    }

    fn ensure_listener(&mut self) -> Result<(), PersistenceError> {
        if self.closure.is_some() {
            return Ok(());
        }

        let window = web_sys::window().ok_or(PersistenceError::recoverable(
            PersistenceErrorKind::BackendUnavailable,
        ))?;
        let local_storage = self.local_storage.clone();
        let session_storage = self.session_storage.clone();

        let closure: Closure<dyn FnMut(StorageEvent)> =
            Closure::wrap(Box::new(move |event: StorageEvent| {
                let Some(area) = event.storage_area() else {
                    return;
                };
                let kind = if local_storage
                    .as_ref()
                    .is_some_and(|storage| Object::is(area.as_ref(), storage.as_ref()))
                {
                    StorageAreaKind::Local
                } else if session_storage
                    .as_ref()
                    .is_some_and(|storage| Object::is(area.as_ref(), storage.as_ref()))
                {
                    StorageAreaKind::Session
                } else {
                    return;
                };

                let Some(key) = event.key() else {
                    return;
                };
                let key: LocalStaticRefStr = key.into();
                let event = match event.new_value() {
                    Some(value) => BackendEvent::Set {
                        key: key.clone(),
                        value,
                    },
                    None => BackendEvent::Removed { key: key.clone() },
                };
                let sinks = STORAGE_HUB.with(|hub| {
                    let hub = hub.borrow();
                    hub.subscribers
                        .get(&(kind, key))
                        .map(|subscribers| {
                            subscribers
                                .iter()
                                .map(|subscriber| subscriber.sink.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                });

                for sink in sinks {
                    sink(event.clone());
                }
            }));

        window
            .add_event_listener_with_callback("storage", closure.as_ref().unchecked_ref())
            .map_err(|error| {
                PersistenceError::recoverable(PersistenceErrorKind::ReadFailed(format!(
                    "add storage listener failed: {:?}",
                    error
                )))
            })?;
        self.closure = Some(closure);
        self.state.mark_attached();
        Ok(())
    }
}

fn report_backend_error(error_sinks: Vec<BackendErrorSinkSlot>, error: PersistenceError) {
    let error_sinks = error_sinks
        .iter()
        .filter_map(|slot| slot.borrow().clone())
        .collect::<Vec<_>>();
    for sink in error_sinks {
        sink(error.clone());
    }
}

fn schedule_storage_detach(generation: u64) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let window_for_callback = window.clone();
    let callback = Closure::once_into_js(move || {
        let Some((closure, error_sinks)) =
            STORAGE_HUB.with(|hub| hub.borrow_mut().begin_remove(generation))
        else {
            return;
        };
        let remove_result = window_for_callback
            .remove_event_listener_with_callback("storage", closure.as_ref().unchecked_ref());
        let removed = remove_result.is_ok();
        let (attach_listener, error_sinks) = STORAGE_HUB.with(|hub| {
            hub.borrow_mut()
                .finish_remove(generation, closure, removed, error_sinks)
        });
        if let Err(error) = remove_result {
            report_backend_error(
                error_sinks,
                PersistenceError::recoverable(PersistenceErrorKind::RemoveFailed(format!(
                    "remove storage listener failed: {:?}",
                    error
                ))),
            );
        }
        if attach_listener {
            let attach_error = STORAGE_HUB.with(|hub| hub.borrow_mut().ensure_listener());
            if let Err(error) = attach_error {
                let error_sinks =
                    STORAGE_HUB.with(|hub| hub.borrow().error_sinks_for_subscribers());
                report_backend_error(error_sinks, error);
            }
        }
    });
    window.queue_microtask(callback.unchecked_ref());
}

fn storage_get(storage: &Storage, key: &str) -> Result<Option<String>, PersistenceError> {
    storage.get_item(key).map_err(|error| {
        PersistenceError::recoverable(PersistenceErrorKind::ReadFailed(format!(
            "storage get_item failed: {:?}",
            error
        )))
    })
}

fn storage_set(storage: &Storage, key: &str, value: &str) -> Result<(), PersistenceError> {
    storage.set_item(key, value).map_err(|error| {
        PersistenceError::recoverable(PersistenceErrorKind::WriteFailed(format!(
            "storage set_item failed: {:?}",
            error
        )))
    })
}

fn storage_remove(storage: &Storage, key: &str) -> Result<(), PersistenceError> {
    storage.remove_item(key).map_err(|error| {
        PersistenceError::recoverable(PersistenceErrorKind::RemoveFailed(format!(
            "storage remove_item failed: {:?}",
            error
        )))
    })
}

fn subscribe_storage(
    kind: StorageAreaKind,
    key: LocalStaticRefStr,
    sink: BackendEventSink,
) -> Result<BackendSubscription<'static>, BackendSubscribeError<'static>> {
    let error_sink = Rc::new(RefCell::new(None));
    let key_for_cleanup = key.clone();
    let error_sink_for_hub = error_sink.clone();
    let id = STORAGE_HUB
        .with(|hub| {
            hub.borrow_mut()
                .subscribe(kind, key, sink, error_sink_for_hub)
        })
        .map_err(BackendSubscribeError::new)?;

    let error_sink_for_cleanup = error_sink.clone();
    Ok(BackendSubscription::with_error_sink_slot(
        move || {
            let generation = STORAGE_HUB.with(|hub| {
                hub.borrow_mut().unsubscribe(
                    kind,
                    key_for_cleanup,
                    id,
                    error_sink_for_cleanup.clone(),
                )
            });
            if let Some(generation) = generation {
                schedule_storage_detach(generation);
            }
        },
        error_sink,
    ))
}

fn storage_handle(kind: StorageAreaKind) -> Result<Storage, PersistenceError> {
    let window = web_sys::window().ok_or(PersistenceError::recoverable(
        PersistenceErrorKind::BackendUnavailable,
    ))?;
    let storage = match kind {
        StorageAreaKind::Local => window.local_storage(),
        StorageAreaKind::Session => window.session_storage(),
    }
    .map_err(|error| {
        PersistenceError::recoverable(PersistenceErrorKind::ReadFailed(format!(
            "storage unavailable: {:?}",
            error
        )))
    })?
    .ok_or(PersistenceError::recoverable(
        PersistenceErrorKind::BackendUnavailable,
    ))?;
    Ok(storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::{OwnerAccess, ReadSignal, Runtime};
    use silex_router::Navigator;
    use std::{cell::RefCell, collections::HashMap};

    #[test]
    fn storage_hub_queues_and_cancels_physical_detach() {
        let mut state = StorageHubState::default();
        state.subscribe();
        assert_eq!(state.listener(), ListenerState::Attached);

        let generation = state
            .unsubscribe()
            .expect("last subscriber should queue detach");
        assert_eq!(state.listener(), ListenerState::DetachQueued(generation));
        state.subscribe();
        assert_eq!(state.listener(), ListenerState::Attached);
        assert!(!state.begin_remove(generation));
    }

    #[test]
    fn storage_hub_reentrant_subscriber_requires_a_new_listener_after_remove() {
        let mut state = StorageHubState::default();
        state.subscribe();
        let generation = state.unsubscribe().expect("detach should be queued");
        assert!(state.begin_remove(generation));
        assert_eq!(state.listener(), ListenerState::Removing(generation));

        state.subscribe();
        assert_eq!(state.listener(), ListenerState::Removing(generation));
        assert!(state.finish_remove(generation, true));
        assert_eq!(state.listener(), ListenerState::Detached);

        state.mark_attached();
        let next_generation = state.unsubscribe().expect("new listener should detach");
        assert!(state.begin_remove(next_generation));
        assert!(!state.finish_remove(next_generation, true));
        assert_eq!(state.listener(), ListenerState::Detached);
    }

    fn test_query_backend<'scope>(
        owner: OwnerAccess<'scope>,
        map: ReadSignal<'scope, HashMap<String, String>>,
    ) -> QueryBackend<'scope> {
        let base_path = owner
            .stored("/".to_string())
            .expect("base path should be stored");
        let (path, set_path) = owner
            .signal("/".to_string())
            .expect("path signal should be created");
        let (search, set_search) = owner
            .signal(String::new())
            .expect("search signal should be created");
        let query_map = map.into_rx();
        let navigator = Navigator {
            base_path,
            path,
            search,
            set_path,
            set_search,
        };
        QueryBackend {
            navigator: Some(navigator),
            query_map: Some(query_map),
        }
    }

    #[test]
    fn query_backend_unavailable_works() {
        let backend = QueryBackend::unavailable();
        assert!(backend.get("key").is_err());
    }

    #[test]
    fn query_backend_get_and_subscribe_follow_query_map_changes() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let map = owner
                    .rw_signal(HashMap::<String, String>::new())
                    .expect("query map signal should be created");
                let backend = test_query_backend(owner, map.read_signal());
                let events = Rc::new(RefCell::new(Vec::<BackendEvent>::new()));
                let callback = {
                    let events = events.clone();
                    Rc::new(move |event| events.borrow_mut().push(event)) as BackendEventSink
                };

                let _subscription = backend
                    .subscribe(
                        owner,
                        "q",
                        callback,
                        owner
                            .error_handler(|_| {})
                            .expect("error handler should be registered")
                            .view(),
                    )
                    .unwrap();
                assert_eq!(backend.get("q").unwrap(), None);

                let mut with_value = HashMap::new();
                with_value.insert("q".to_string(), "rust".to_string());
                map.set(with_value).expect("query map should update");

                assert_eq!(backend.get("q").unwrap(), Some("rust".to_string()));
                assert!(matches!(
                    events.borrow().first(),
                    Some(BackendEvent::Set { key, value }) if key == "q" && value == "rust"
                ));

                map.set(HashMap::new()).expect("query map should update");
                assert!(matches!(
                    events.borrow().get(1),
                    Some(BackendEvent::Removed { key }) if key == "q"
                ));
            })
            .expect("query backend test owner should run");
    }

    #[test]
    fn query_backend_unavailable_reports_backend_unavailable() {
        assert!(matches!(
            QueryBackend::<'static>::unavailable().get("q"),
            Err(PersistenceError::Recoverable(
                PersistenceErrorKind::BackendUnavailable,
            ))
        ));

        let mut runtime = Runtime::new();
        runtime
            .with_transient(|owner| {
                let backend = QueryBackend::unavailable();
                let result = backend
                    .subscribe(
                        owner,
                        "q",
                        Rc::new(|_| {}),
                        owner
                            .error_handler(|_| {})
                            .expect("error handler should be registered")
                            .view(),
                    )
                    .map_err(|error| error.into_error());
                assert!(matches!(
                    result,
                    Err(PersistenceError::Recoverable(
                        PersistenceErrorKind::BackendUnavailable
                    ))
                ));
            })
            .expect("unavailable backend test owner should run");
    }

    #[test]
    fn query_backend_rejects_a_foreign_tracked_subscription() {
        let mut first_runtime = Runtime::new();
        let first_root = first_runtime
            .owner()
            .expect("first owner should be created");
        let mut second_runtime = Runtime::new();
        let second_root = second_runtime
            .owner()
            .expect("second owner should be created");
        let owner = first_root.access();
        let target_owner = second_root.access();
        let map = owner
            .rw_signal(HashMap::<String, String>::new())
            .expect("query map signal should be created");
        let backend = test_query_backend(owner, map.read_signal());
        let result = backend
            .subscribe(
                target_owner,
                "q",
                Rc::new(|_| {}),
                target_owner
                    .error_handler(|_| {})
                    .expect("error handler should be registered")
                    .view(),
            )
            .map_err(|error| error.into_error());
        assert!(matches!(
            result,
            Err(PersistenceError::Fatal(
                PersistenceErrorKind::InvalidConfiguration(_)
            ))
        ));

        second_root.close().expect("close second owner");
        first_root.close().expect("close first owner");
    }
}
