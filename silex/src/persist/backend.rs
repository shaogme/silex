use crate::persist::PersistenceError;
use js_sys::Object;
use silex_core::reactivity::{Effect, Memo, NodeId, create_scope, dispose};
use silex_core::traits::RxGet;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Storage, StorageEvent};

#[derive(Clone, Debug, PartialEq)]
pub enum BackendEvent {
    Set { key: String, value: String },
    Removed { key: String },
    ExternalRefresh,
}

pub struct BackendSubscription {
    cleanup: Option<Box<dyn FnOnce()>>,
}

impl BackendSubscription {
    pub fn new(cleanup: impl FnOnce() + 'static) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
        }
    }
}

impl Drop for BackendSubscription {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

pub trait PersistenceBackend: Clone + 'static {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError>;
    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError>;
    fn remove(&self, key: &str) -> Result<(), PersistenceError>;
    fn subscribe(
        &self,
        key: String,
        callback: Rc<dyn Fn(BackendEvent)>,
    ) -> Result<BackendSubscription, PersistenceError>;
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
        // All instances of the same backend type are conceptually equal
        true
    }
}

impl<const IS_LOCAL: bool> Eq for WebStorageBackend<IS_LOCAL> {}

pub type LocalStorageBackend = WebStorageBackend<true>;
pub type SessionStorageBackend = WebStorageBackend<false>;

#[derive(Clone)]
pub struct QueryBackend {
    navigator: Option<crate::router::Navigator>,
    query_map: Option<Memo<std::collections::HashMap<String, String>>>,
}

impl QueryBackend {
    pub fn new() -> Result<Self, PersistenceError> {
        let router = crate::router::use_router().ok_or(PersistenceError::BackendUnavailable)?;
        Ok(Self {
            navigator: Some(router.navigator),
            query_map: Some(crate::router::use_query_map()),
        })
    }

    pub fn unavailable() -> Self {
        Self {
            navigator: None,
            query_map: None,
        }
    }

    fn navigator(&self) -> Result<&crate::router::Navigator, PersistenceError> {
        self.navigator
            .as_ref()
            .ok_or(PersistenceError::BackendUnavailable)
    }

    fn query_map(
        &self,
    ) -> Result<Memo<std::collections::HashMap<String, String>>, PersistenceError> {
        self.query_map.ok_or(PersistenceError::BackendUnavailable)
    }
}

impl<const IS_LOCAL: bool> PersistenceBackend for WebStorageBackend<IS_LOCAL> {
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
        key: String,
        callback: Rc<dyn Fn(BackendEvent)>,
    ) -> Result<BackendSubscription, PersistenceError> {
        subscribe_storage(Self::kind(), key, callback)
    }
}

impl PersistenceBackend for QueryBackend {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        Ok(self.query_map()?.get_untracked().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        self.navigator()?.set_query(key, Some(value));
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        self.navigator()?.set_query(key, None);
        Ok(())
    }

    fn subscribe(
        &self,
        key: String,
        callback: Rc<dyn Fn(BackendEvent)>,
    ) -> Result<BackendSubscription, PersistenceError> {
        let query_map = self.query_map()?;
        let scope_id: NodeId = create_scope(move || {
            Effect::new(move |prev: Option<Option<String>>| {
                let current = query_map.get().get(&key).cloned();
                if let Some(previous) = prev
                    && previous != current
                {
                    match current.clone() {
                        Some(value) => callback(BackendEvent::Set {
                            key: key.clone(),
                            value,
                        }),
                        None => callback(BackendEvent::Removed { key: key.clone() }),
                    }
                }
                current
            });
        });
        Ok(BackendSubscription::new(move || dispose(scope_id)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum StorageAreaKind {
    Local,
    Session,
}

struct StorageSubscriber {
    id: usize,
    callback: Rc<dyn Fn(BackendEvent)>,
}

struct StorageDispatcher {
    subscribers: HashMap<(StorageAreaKind, String), Vec<StorageSubscriber>>,
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
            local_storage: web_sys::window().and_then(|w| w.local_storage().ok().flatten()),
            session_storage: web_sys::window().and_then(|w| w.session_storage().ok().flatten()),
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
        key: String,
        callback: Rc<dyn Fn(BackendEvent)>,
    ) -> Result<usize, PersistenceError> {
        self.ensure_listener()?;

        let id = self.next_id;
        self.next_id += 1;

        self.subscribers
            .entry((kind, key))
            .or_default()
            .push(StorageSubscriber { id, callback });

        Ok(id)
    }

    fn unsubscribe(&mut self, kind: StorageAreaKind, key: &str, id: usize) {
        if let Some(subs) = self.subscribers.get_mut(&(kind, key.to_string())) {
            subs.retain(|s| s.id != id);
            if subs.is_empty() {
                self.subscribers.remove(&(kind, key.to_string()));
            }
        }

        if self.subscribers.is_empty()
            && let Some(closure) = self.closure.take()
            && let Some(window) = web_sys::window()
        {
            let _ = window
                .remove_event_listener_with_callback("storage", closure.as_ref().unchecked_ref());
        }
    }

    fn ensure_listener(&mut self) -> Result<(), PersistenceError> {
        if self.closure.is_some() {
            return Ok(());
        }

        let window = web_sys::window().ok_or(PersistenceError::BackendUnavailable)?;

        let local_storage = self.local_storage.clone();
        let session_storage = self.session_storage.clone();

        let closure = Closure::wrap(Box::new(move |event: StorageEvent| {
            let Some(area) = event.storage_area() else {
                return;
            };

            let kind = if local_storage
                .as_ref()
                .is_some_and(|l| Object::is(area.as_ref(), l.as_ref()))
            {
                StorageAreaKind::Local
            } else if session_storage
                .as_ref()
                .is_some_and(|s| Object::is(area.as_ref(), s.as_ref()))
            {
                StorageAreaKind::Session
            } else {
                return;
            };

            let Some(key) = event.key() else {
                return;
            };
            let new_value = event.new_value();

            DISPATCHER.with(|d| {
                if let Some(subs) = d.borrow().subscribers.get(&(kind, key.clone())) {
                    let event = match new_value {
                        Some(value) => BackendEvent::Set {
                            key: key.clone(),
                            value,
                        },
                        None => BackendEvent::Removed { key: key.clone() },
                    };
                    for sub in subs {
                        (sub.callback)(event.clone());
                    }
                }
            });
        }) as Box<dyn FnMut(StorageEvent)>);

        window
            .add_event_listener_with_callback("storage", closure.as_ref().unchecked_ref())
            .map_err(|err| {
                PersistenceError::ReadFailed(format!("add storage listener failed: {:?}", err))
            })?;

        self.closure = Some(closure);
        Ok(())
    }
}

fn storage_get(storage: &Storage, key: &str) -> Result<Option<String>, PersistenceError> {
    storage
        .get_item(key)
        .map_err(|err| PersistenceError::ReadFailed(format!("storage get_item failed: {:?}", err)))
}

fn storage_set(storage: &Storage, key: &str, value: &str) -> Result<(), PersistenceError> {
    storage
        .set_item(key, value)
        .map_err(|err| PersistenceError::WriteFailed(format!("storage set_item failed: {:?}", err)))
}

fn storage_remove(storage: &Storage, key: &str) -> Result<(), PersistenceError> {
    storage.remove_item(key).map_err(|err| {
        PersistenceError::RemoveFailed(format!("storage remove_item failed: {:?}", err))
    })
}

fn subscribe_storage(
    kind: StorageAreaKind,
    key: String,
    callback: Rc<dyn Fn(BackendEvent)>,
) -> Result<BackendSubscription, PersistenceError> {
    let id = DISPATCHER.with(|d| d.borrow_mut().subscribe(kind, key.clone(), callback))?;

    Ok(BackendSubscription::new(move || {
        DISPATCHER.with(|d| d.borrow_mut().unsubscribe(kind, &key, id));
    }))
}

fn storage_handle(kind: StorageAreaKind) -> Result<Storage, PersistenceError> {
    let window = web_sys::window().ok_or(PersistenceError::BackendUnavailable)?;
    let storage = match kind {
        StorageAreaKind::Local => window.local_storage(),
        StorageAreaKind::Session => window.session_storage(),
    }
    .map_err(|err| PersistenceError::ReadFailed(format!("storage unavailable: {:?}", err)))?
    .ok_or(PersistenceError::BackendUnavailable)?;
    Ok(storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::Navigator;
    use silex_core::reactivity::{Signal, create_scope};
    use silex_core::traits::RxWrite;
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn test_query_backend(
        map: silex_core::reactivity::ReadSignal<HashMap<String, String>>,
    ) -> QueryBackend {
        let (path, set_path) = Signal::pair("/".to_string());
        let (search, set_search) = Signal::pair(String::new());
        QueryBackend {
            navigator: Some(Navigator {
                base_path: "/".to_string(),
                path,
                search,
                set_path,
                set_search,
            }),
            query_map: Some(Memo::new(move |_| map.get())),
        }
    }

    #[test]
    fn query_backend_new_without_router_is_unavailable() {
        assert!(matches!(
            QueryBackend::new(),
            Err(PersistenceError::BackendUnavailable)
        ));
    }

    #[test]
    fn query_backend_get_and_subscribe_follow_query_map_changes() {
        create_scope(|| {
            let (map, set_map) = Signal::pair(HashMap::<String, String>::new());
            let backend = test_query_backend(map);
            let events = Rc::new(RefCell::new(Vec::<BackendEvent>::new()));

            let callback = {
                let events = events.clone();
                Rc::new(move |event| events.borrow_mut().push(event))
            };

            let _subscription = backend.subscribe("q".to_string(), callback).unwrap();
            assert_eq!(backend.get("q").unwrap(), None);

            let mut with_value = HashMap::new();
            with_value.insert("q".to_string(), "rust".to_string());
            set_map.set(with_value);

            assert_eq!(backend.get("q").unwrap(), Some("rust".to_string()));
            assert!(matches!(
                events.borrow().first(),
                Some(BackendEvent::Set { key, value }) if key == "q" && value == "rust"
            ));

            set_map.set(HashMap::new());
            assert!(matches!(
                events.borrow().get(1),
                Some(BackendEvent::Removed { key }) if key == "q"
            ));
        });
    }

    #[test]
    fn query_backend_unavailable_reports_backend_unavailable() {
        let backend = QueryBackend::unavailable();

        assert!(matches!(
            backend.get("q"),
            Err(PersistenceError::BackendUnavailable)
        ));
        assert!(matches!(
            backend.subscribe("q".to_string(), Rc::new(|_| {})),
            Err(PersistenceError::BackendUnavailable)
        ));
    }
}
