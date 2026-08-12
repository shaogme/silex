use crate::PersistenceError;
use js_sys::Object;
use ref_str::LocalStaticRefStr;
use silex_core::{
    ErrorReporter, RuntimeInputs, Rx, Scope, SilexResult, reactivity::runtime_inputs_of,
};
use silex_router::{Navigator, RouterContext};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Storage, StorageEvent};

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

pub struct BackendSubscription<'scope> {
    cleanup: Option<Box<dyn FnOnce() + 'scope>>,
}

impl<'scope> BackendSubscription<'scope> {
    pub fn new(cleanup: impl FnOnce() + 'scope) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
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

    fn runtime_inputs(&self) -> RuntimeInputs {
        RuntimeInputs::new()
    }

    fn subscribe(
        &self,
        scope: Scope<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>>;
}

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
        self.storage
            .as_ref()
            .ok_or(PersistenceError::BackendUnavailable)
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
    inputs: RuntimeInputs,
}

impl<'scope> QueryBackend<'scope> {
    pub fn new(ctx: RouterContext<'scope>) -> Self {
        let navigator = ctx.navigator;
        let query_map = ctx.query_map();
        let mut inputs = RuntimeInputs::new();
        inputs.extend(&runtime_inputs_of(ctx.base_path));
        inputs.extend(&runtime_inputs_of(ctx.path));
        inputs.extend(&runtime_inputs_of(ctx.search));
        inputs.extend(&runtime_inputs_of(navigator.path));
        inputs.extend(&runtime_inputs_of(navigator.search));
        inputs.extend(&runtime_inputs_of(query_map));

        Self {
            navigator: Some(navigator),
            query_map: Some(query_map),
            inputs,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            navigator: None,
            query_map: None,
            inputs: RuntimeInputs::new(),
        }
    }

    fn navigator(&self) -> Result<&Navigator<'scope>, PersistenceError> {
        self.navigator
            .as_ref()
            .ok_or(PersistenceError::BackendUnavailable)
    }

    fn query_map(&self) -> Result<Rx<'scope, HashMap<String, String>>, PersistenceError> {
        self.query_map.ok_or(PersistenceError::BackendUnavailable)
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
        _scope: Scope<'scope>,
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

    fn runtime_inputs(&self) -> RuntimeInputs {
        self.inputs.clone()
    }

    fn subscribe(
        &self,
        scope: Scope<'scope>,
        key: impl Into<LocalStaticRefStr>,
        sink: BackendEventSink,
        error_handler: ErrorReporter<'scope>,
    ) -> Result<BackendSubscription<'scope>, BackendSubscribeError<'scope>> {
        let inputs = self.runtime_inputs();
        scope.validate_inputs(&inputs).map_err(|error| {
            BackendSubscribeError::new(PersistenceError::InvalidConfiguration(error.to_string()))
        })?;

        let key = key.into();
        let query_map = self.query_map().map_err(BackendSubscribeError::new)?;
        let active = Rc::new(Cell::new(true));
        let active_for_effect = active.clone();
        let key_for_effect = key.clone();
        let _effect = scope
            .effect_with_previous_from(
                inputs,
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
                BackendSubscribeError::new(PersistenceError::InvalidConfiguration(
                    error.to_string(),
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

struct StorageSubscriber {
    id: usize,
    sink: BackendEventSink,
}

struct StorageDispatcher {
    subscribers: HashMap<(StorageAreaKind, LocalStaticRefStr), Vec<StorageSubscriber>>,
    next_id: usize,
    closure: Option<Closure<dyn FnMut(StorageEvent)>>,
    local_storage: Option<Storage>,
    session_storage: Option<Storage>,
}

impl Default for StorageDispatcher {
    fn default() -> Self {
        Self {
            subscribers: HashMap::new(),
            next_id: 0,
            closure: None,
            local_storage: web_sys::window()
                .and_then(|window| window.local_storage().ok().flatten()),
            session_storage: web_sys::window()
                .and_then(|window| window.session_storage().ok().flatten()),
        }
    }
}

thread_local! {
    static DISPATCHER: RefCell<StorageDispatcher> = RefCell::new(StorageDispatcher::default());
}

impl StorageDispatcher {
    fn subscribe(
        &mut self,
        kind: StorageAreaKind,
        key: LocalStaticRefStr,
        sink: BackendEventSink,
    ) -> Result<usize, PersistenceError> {
        self.ensure_listener()?;

        let id = self.next_id;
        self.next_id += 1;
        self.subscribers
            .entry((kind, key))
            .or_default()
            .push(StorageSubscriber { id, sink });
        Ok(id)
    }

    fn unsubscribe(
        &mut self,
        kind: StorageAreaKind,
        key: impl Into<LocalStaticRefStr>,
        id: usize,
    ) -> Option<Closure<dyn FnMut(StorageEvent)>> {
        let key = key.into();
        if let Some(subscribers) = self.subscribers.get_mut(&(kind, key.clone())) {
            subscribers.retain(|subscriber| subscriber.id != id);
            if subscribers.is_empty() {
                self.subscribers.remove(&(kind, key));
            }
        }

        if self.subscribers.is_empty() {
            self.closure.take()
        } else {
            None
        }
    }

    fn ensure_listener(&mut self) -> Result<(), PersistenceError> {
        if self.closure.is_some() {
            return Ok(());
        }

        let window = web_sys::window().ok_or(PersistenceError::BackendUnavailable)?;
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
                let sinks = DISPATCHER.with(|dispatcher| {
                    let dispatcher = dispatcher.borrow();
                    dispatcher
                        .subscribers
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
                PersistenceError::ReadFailed(format!("add storage listener failed: {:?}", error))
            })?;
        self.closure = Some(closure);
        Ok(())
    }
}

fn storage_get(storage: &Storage, key: &str) -> Result<Option<String>, PersistenceError> {
    storage.get_item(key).map_err(|error| {
        PersistenceError::ReadFailed(format!("storage get_item failed: {:?}", error))
    })
}

fn storage_set(storage: &Storage, key: &str, value: &str) -> Result<(), PersistenceError> {
    storage.set_item(key, value).map_err(|error| {
        PersistenceError::WriteFailed(format!("storage set_item failed: {:?}", error))
    })
}

fn storage_remove(storage: &Storage, key: &str) -> Result<(), PersistenceError> {
    storage.remove_item(key).map_err(|error| {
        PersistenceError::RemoveFailed(format!("storage remove_item failed: {:?}", error))
    })
}

fn subscribe_storage(
    kind: StorageAreaKind,
    key: LocalStaticRefStr,
    sink: BackendEventSink,
) -> Result<BackendSubscription<'static>, BackendSubscribeError<'static>> {
    let key_for_cleanup = key.clone();
    let id = DISPATCHER
        .with(|dispatcher| dispatcher.borrow_mut().subscribe(kind, key, sink))
        .map_err(BackendSubscribeError::new)?;

    Ok(BackendSubscription::new(move || {
        let closure = DISPATCHER.with(|dispatcher| {
            dispatcher
                .borrow_mut()
                .unsubscribe(kind, key_for_cleanup, id)
        });
        if let Some(closure) = closure
            && let Some(window) = web_sys::window()
        {
            let _ = window
                .remove_event_listener_with_callback("storage", closure.as_ref().unchecked_ref());
        }
    }))
}

fn storage_handle(kind: StorageAreaKind) -> Result<Storage, PersistenceError> {
    let window = web_sys::window().ok_or(PersistenceError::BackendUnavailable)?;
    let storage = match kind {
        StorageAreaKind::Local => window.local_storage(),
        StorageAreaKind::Session => window.session_storage(),
    }
    .map_err(|error| PersistenceError::ReadFailed(format!("storage unavailable: {:?}", error)))?
    .ok_or(PersistenceError::BackendUnavailable)?;
    Ok(storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::{ReadSignal, Runtime};
    use silex_router::Navigator;
    use std::{cell::RefCell, collections::HashMap};

    fn test_query_backend<'scope>(
        scope: Scope<'scope>,
        map: ReadSignal<'scope, HashMap<String, String>>,
    ) -> QueryBackend<'scope> {
        let base_path = scope
            .stored("/".to_string())
            .expect("base path should be stored");
        let (path, set_path) = scope
            .signal("/".to_string())
            .expect("path signal should be created");
        let (search, set_search) = scope
            .signal(String::new())
            .expect("search signal should be created");
        let query_map = scope
            .derived_from(
                runtime_inputs_of(map),
                move || map.get(),
                scope
                    .error_handler(|_| {})
                    .expect("error handler should be registered"),
            )
            .expect("query map should be derived");
        let navigator = Navigator {
            base_path,
            path,
            search,
            set_path,
            set_search,
        };
        let mut inputs = RuntimeInputs::new();
        inputs.extend(&runtime_inputs_of(base_path));
        inputs.extend(&runtime_inputs_of(path));
        inputs.extend(&runtime_inputs_of(search));
        inputs.extend(&runtime_inputs_of(query_map));
        QueryBackend {
            navigator: Some(navigator),
            query_map: Some(query_map),
            inputs,
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
            .child(|scope| {
                let map = scope
                    .rw_signal(HashMap::<String, String>::new())
                    .expect("query map signal should be created");
                let backend = test_query_backend(scope, map.read_signal());
                let events = Rc::new(RefCell::new(Vec::<BackendEvent>::new()));
                let callback = {
                    let events = events.clone();
                    Rc::new(move |event| events.borrow_mut().push(event)) as BackendEventSink
                };

                let _subscription = backend
                    .subscribe(
                        scope,
                        "q",
                        callback,
                        scope
                            .error_handler(|_| {})
                            .expect("error handler should be registered"),
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
            .expect("query backend test scope should run");
    }

    #[test]
    fn query_backend_unavailable_reports_backend_unavailable() {
        assert!(matches!(
            QueryBackend::<'static>::unavailable().get("q"),
            Err(PersistenceError::BackendUnavailable)
        ));

        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let backend = QueryBackend::unavailable();
                let result = backend
                    .subscribe(
                        scope,
                        "q",
                        Rc::new(|_| {}),
                        scope
                            .error_handler(|_| {})
                            .expect("error handler should be registered"),
                    )
                    .map_err(|error| error.into_error());
                assert!(matches!(result, Err(PersistenceError::BackendUnavailable)));
            })
            .expect("unavailable backend test scope should run");
    }

    #[test]
    fn query_runtime_inputs_reject_a_foreign_scope_before_effect_creation() {
        let mut first_runtime = Runtime::new();
        let first_root = first_runtime.run().expect("first root should be created");
        let inputs = first_root.with_scope(|scope| {
            let map = scope
                .rw_signal(HashMap::<String, String>::new())
                .expect("query map signal should be created");
            test_query_backend(scope, map.read_signal()).runtime_inputs()
        });

        let mut second_runtime = Runtime::new();
        let second_root = second_runtime.run().expect("second root should be created");
        second_root.with_scope(|scope| {
            assert!(scope.validate_inputs(&inputs).is_err());
            let runs = Rc::new(Cell::new(0));
            let runs_for_effect = runs.clone();
            assert!(
                scope
                    .effect_from(
                        inputs,
                        move || -> SilexResult<()> {
                            runs_for_effect.set(runs_for_effect.get() + 1);
                            Ok(())
                        },
                        scope
                            .error_handler(|_| {})
                            .expect("error handler should be registered"),
                    )
                    .is_err()
            );
            assert_eq!(runs.get(), 0);
        });

        second_root.dispose().expect("dispose second root");
        first_root.dispose().expect("dispose first root");
    }
}
