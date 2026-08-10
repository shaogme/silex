use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
};

use silex_core::{CompletionOnce, ErrorReporter, Scope, SilexError, SilexResult, unwind_safe};
use silex_persist::{LocalStorageBackend, PersistCodec, PersistenceBackend};

use crate::{
    codec::CacheCodec,
    state::{CacheConfig, CacheEviction, RequestSpec},
};

#[derive(Clone)]
pub(crate) struct CacheBinding<T> {
    pub(crate) key: String,
    pub(crate) token: Rc<Cell<bool>>,
    pub(crate) snapshot: Option<T>,
}

struct CacheEntry<T> {
    snapshot: Option<T>,
    token: Rc<Cell<bool>>,
    last_access: u64,
    last_access_at: f64,
}

type CacheEntries<T> = Rc<RefCell<HashMap<String, CacheEntry<T>>>>;
type CacheEncode<T> = Rc<dyn Fn(&T) -> Result<String, String>>;
type CacheDecode<T> = Rc<dyn Fn(&str) -> Result<T, String>>;

pub(crate) trait CacheRuntime<'scope, T>: 'scope {
    fn binding(&self, spec: &RequestSpec) -> Option<CacheBinding<T>>;

    fn cached_value(&self, spec: &RequestSpec) -> Option<T>;

    fn completion_once_for_binding(
        &self,
        scope: Scope<'scope>,
        binding: CacheBinding<T>,
    ) -> CompletionOnce<T>;
}

pub(crate) struct CacheRuntimeImpl<'scope, T> {
    config: CacheConfig,
    entries: CacheEntries<T>,
    next_access: Cell<u64>,
    backend: LocalStorageBackend,
    encode: CacheEncode<T>,
    decode: CacheDecode<T>,
    _scope: PhantomData<&'scope ()>,
}

impl<'scope, T> CacheRuntimeImpl<'scope, T>
where
    T: Clone + PartialEq + 'static,
{
    pub(crate) fn new<C>(config: CacheConfig, codec: C) -> Self
    where
        C: CacheCodec<T>,
    {
        let encoder = codec.clone();
        let decoder = codec;
        Self {
            config,
            entries: Rc::new(RefCell::new(HashMap::new())),
            next_access: Cell::new(0),
            backend: LocalStorageBackend::default(),
            encode: Rc::new(move |value| encoder.encode(value)),
            decode: Rc::new(move |raw| PersistCodec::decode(&decoder, raw)),
            _scope: PhantomData,
        }
    }

    pub(crate) fn register_cleanup(
        &self,
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<()> {
        let entries = self.entries.clone();
        scope.on_cleanup(
            move || {
                for entry in entries.borrow_mut().drain().map(|(_, entry)| entry) {
                    entry.token.set(false);
                }
                Ok(())
            },
            error_handler,
        )
    }

    fn key(spec: &RequestSpec) -> String {
        format!("__net_cache_{}__", spec.cache_key())
    }

    fn now() -> f64 {
        js_sys::Date::now()
    }

    fn touch(&self, entry: &mut CacheEntry<T>) {
        let access = self
            .next_access
            .get()
            .checked_add(1)
            .expect("HTTP cache access counter exhausted");
        self.next_access.set(access);
        entry.last_access = access;
        entry.last_access_at = Self::now();
    }

    fn expired(&self, entry: &CacheEntry<T>) -> bool {
        self.config
            .ttl
            .is_some_and(|ttl| Self::now() - entry.last_access_at >= ttl.as_millis() as f64)
    }

    fn remove_persisted(&self, key: &str) {
        if matches!(self.config.eviction, CacheEviction::RemovePersisted) {
            let _ = self.backend.remove(key);
        }
    }

    fn evict_one(&self) {
        let key = {
            let entries = self.entries.borrow();
            entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
        };
        let Some(key) = key else {
            return;
        };
        if let Some(entry) = self.entries.borrow_mut().remove(&key) {
            entry.token.set(false);
            self.remove_persisted(&key);
        }
    }

    fn load_snapshot(&self, key: &str) -> Option<T> {
        let raw = match self.backend.get(key) {
            Ok(Some(raw)) => raw,
            Ok(None) | Err(_) => return None,
        };
        match (self.decode)(&raw) {
            Ok(value) => Some(value),
            Err(_) => {
                self.remove_persisted(key);
                None
            }
        }
    }

    fn ensure_entry(&self, key: &str) -> Option<Option<T>> {
        if self.config.capacity == 0 {
            return None;
        }

        let expired = self
            .entries
            .borrow()
            .get(key)
            .is_some_and(|entry| self.expired(entry));
        if expired && let Some(entry) = self.entries.borrow_mut().remove(key) {
            entry.token.set(false);
            self.remove_persisted(key);
        }

        if self.entries.borrow().contains_key(key) {
            let mut entries = self.entries.borrow_mut();
            let entry = entries
                .get_mut(key)
                .expect("cache entry exists while mutably borrowed");
            self.touch(entry);
            return Some(entry.snapshot.clone());
        }

        if self.entries.borrow().len() >= self.config.capacity {
            self.evict_one();
        }

        let access = self
            .next_access
            .get()
            .checked_add(1)
            .expect("HTTP cache access counter exhausted");
        self.next_access.set(access);
        let entry = CacheEntry {
            snapshot: self.load_snapshot(key),
            token: Rc::new(Cell::new(true)),
            last_access: access,
            last_access_at: Self::now(),
        };
        let snapshot = entry.snapshot.clone();
        let mut entries = self.entries.borrow_mut();
        entries.insert(key.to_string(), entry);
        Some(snapshot)
    }

    fn begin_binding(&self, key: &str, snapshot: Option<T>) -> CacheBinding<T> {
        let token = Rc::new(Cell::new(true));
        let mut entries = self.entries.borrow_mut();
        let entry = entries
            .get_mut(key)
            .expect("cache entry must exist before binding");
        entry.token.set(false);
        entry.token = token.clone();
        self.touch(entry);
        CacheBinding {
            key: key.to_string(),
            token,
            snapshot,
        }
    }
}

impl<'scope, T> CacheRuntime<'scope, T> for CacheRuntimeImpl<'scope, T>
where
    T: Clone + PartialEq + 'static,
{
    fn binding(&self, spec: &RequestSpec) -> Option<CacheBinding<T>> {
        let key = Self::key(spec);
        let snapshot = self.ensure_entry(&key)?;
        let binding = self.begin_binding(&key, snapshot);
        Some(binding)
    }

    fn cached_value(&self, spec: &RequestSpec) -> Option<T> {
        self.ensure_entry(&Self::key(spec))?
    }

    fn completion_once_for_binding(
        &self,
        scope: Scope<'scope>,
        binding: CacheBinding<T>,
    ) -> CompletionOnce<T> {
        let entries = self.entries.clone();
        let backend = self.backend.clone();
        let encode = self.encode.clone();
        let key = binding.key;
        let token = binding.token;
        scope.completion_once(unwind_safe(move |value: T| {
            if !token.get() {
                return Ok(());
            }
            let raw = encode(&value).map_err(|error| {
                SilexError::Framework(format!("encode HTTP cache value failed: {error}"))
            })?;
            {
                let mut entries = entries.borrow_mut();
                let Some(entry) = entries.get_mut(&key) else {
                    return Ok(());
                };
                if !Rc::ptr_eq(&entry.token, &token) || !token.get() {
                    return Ok(());
                }
                entry.snapshot = Some(value);
            }
            backend.set(&key, &raw).map_err(|error| {
                SilexError::Framework(format!("write HTTP cache value failed: {error}"))
            })?;
            Ok(())
        }))
    }
}
