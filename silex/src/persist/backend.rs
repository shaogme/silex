use crate::persist::PersistenceError;
use js_sys::Object;
use silex_core::reactivity::{Effect, Memo, NodeId, create_scope, dispose};
use silex_core::traits::RxGet;
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalStorageBackend;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionStorageBackend;

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

impl PersistenceBackend for LocalStorageBackend {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        storage_get(StorageAreaKind::Local, key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        storage_set(StorageAreaKind::Local, key, value)
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        storage_remove(StorageAreaKind::Local, key)
    }

    fn subscribe(
        &self,
        key: String,
        callback: Rc<dyn Fn(BackendEvent)>,
    ) -> Result<BackendSubscription, PersistenceError> {
        subscribe_storage(StorageAreaKind::Local, key, callback)
    }
}

impl PersistenceBackend for SessionStorageBackend {
    fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        storage_get(StorageAreaKind::Session, key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
        storage_set(StorageAreaKind::Session, key, value)
    }

    fn remove(&self, key: &str) -> Result<(), PersistenceError> {
        storage_remove(StorageAreaKind::Session, key)
    }

    fn subscribe(
        &self,
        key: String,
        callback: Rc<dyn Fn(BackendEvent)>,
    ) -> Result<BackendSubscription, PersistenceError> {
        subscribe_storage(StorageAreaKind::Session, key, callback)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorageAreaKind {
    Local,
    Session,
}

fn storage_get(kind: StorageAreaKind, key: &str) -> Result<Option<String>, PersistenceError> {
    storage_handle(kind)?
        .get_item(key)
        .map_err(|err| PersistenceError::ReadFailed(format!("storage get_item failed: {:?}", err)))
}

fn storage_set(kind: StorageAreaKind, key: &str, value: &str) -> Result<(), PersistenceError> {
    storage_handle(kind)?
        .set_item(key, value)
        .map_err(|err| PersistenceError::WriteFailed(format!("storage set_item failed: {:?}", err)))
}

fn storage_remove(kind: StorageAreaKind, key: &str) -> Result<(), PersistenceError> {
    storage_handle(kind)?.remove_item(key).map_err(|err| {
        PersistenceError::RemoveFailed(format!("storage remove_item failed: {:?}", err))
    })
}

fn subscribe_storage(
    kind: StorageAreaKind,
    key: String,
    callback: Rc<dyn Fn(BackendEvent)>,
) -> Result<BackendSubscription, PersistenceError> {
    let window = web_sys::window().ok_or(PersistenceError::BackendUnavailable)?;
    let storage = storage_handle(kind)?;
    let target_key = key.clone();
    let storage_clone = storage.clone();
    let closure = Closure::wrap(Box::new(move |event: StorageEvent| {
        if event.key().as_deref() != Some(target_key.as_str()) {
            return;
        }

        let Some(area) = event.storage_area() else {
            return;
        };

        if !Object::is(area.as_ref(), storage_clone.as_ref()) {
            return;
        }

        match event.new_value() {
            Some(value) => callback(BackendEvent::Set {
                key: target_key.clone(),
                value,
            }),
            None => callback(BackendEvent::Removed {
                key: target_key.clone(),
            }),
        }
    }) as Box<dyn FnMut(StorageEvent)>);

    window
        .add_event_listener_with_callback("storage", closure.as_ref().unchecked_ref())
        .map_err(|err| {
            PersistenceError::ReadFailed(format!("add storage listener failed: {:?}", err))
        })?;

    Ok(BackendSubscription::new(move || {
        if let Some(window) = web_sys::window() {
            let _ = window
                .remove_event_listener_with_callback("storage", closure.as_ref().unchecked_ref());
        }
        drop(closure);
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
    use silex_core::reactivity::{create_scope, signal};
    use silex_core::traits::RxWrite;
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn test_query_backend(
        map: silex_core::reactivity::ReadSignal<HashMap<String, String>>,
    ) -> QueryBackend {
        let (path, set_path) = signal("/".to_string());
        let (search, set_search) = signal(String::new());
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
            let (map, set_map) = signal(HashMap::<String, String>::new());
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
