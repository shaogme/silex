use std::{collections::HashMap, rc::Rc};

use silex_core::{CompletionOnce, Scope, SilexError, StoredValue, unwind_safe};
use silex_persist::{LocalStorageBackend, PersistCodec, PersistenceBackend};

use crate::{
    codec::CacheCodec,
    state::{CacheConfig, CacheEviction, RequestSpec},
};

#[derive(Clone)]
pub(crate) struct CacheBinding<T> {
    pub(crate) key: String,
    pub(crate) generation: u64,
    pub(crate) snapshot: Option<T>,
}

struct CacheEntry<T> {
    snapshot: Option<T>,
    generation: u64,
    last_access: u64,
    last_access_at: f64,
}

type CacheEncode<T> = Rc<dyn Fn(&T) -> Result<String, String>>;
type CacheDecode<T> = Rc<dyn Fn(&str) -> Result<T, String>>;

struct CacheState<T> {
    config: CacheConfig,
    entries: HashMap<String, CacheEntry<T>>,
    next_access: u64,
    next_generation: u64,
    backend: LocalStorageBackend,
    encode: CacheEncode<T>,
    decode: CacheDecode<T>,
}

/// A cache whose state and completion tickets belong to one reactive scope.
///
/// The handle is separate from `HttpClientBuilder`: several builders can
/// share it, while a builder cannot create a second cache with a different
/// codec or lifecycle by accident.
#[derive(Clone, Copy)]
pub struct HttpCache<'scope, T> {
    state: StoredValue<'scope, CacheState<T>>,
    scope: Scope<'scope>,
}

impl<'scope, T> HttpCache<'scope, T>
where
    T: Clone + 'static,
{
    pub fn new<C>(scope: Scope<'scope>, config: CacheConfig, codec: C) -> Self
    where
        C: CacheCodec<T>,
    {
        let encoder = codec.clone();
        let decoder = codec;
        let state = scope.stored(CacheState {
            config,
            entries: HashMap::new(),
            next_access: 0,
            next_generation: 0,
            backend: LocalStorageBackend::default(),
            encode: Rc::new(move |value| encoder.encode(value)),
            decode: Rc::new(move |raw| PersistCodec::decode(&decoder, raw)),
        });
        Self { state, scope }
    }

    pub(crate) fn belongs_to(&self, scope: Scope<'scope>) -> bool {
        self.scope == scope
    }

    fn key(spec: &RequestSpec) -> String {
        format!("__net_cache_{}__", spec.cache_key())
    }

    fn now() -> f64 {
        js_sys::Date::now()
    }

    fn touch(entry: &mut CacheEntry<T>, next_access: &mut u64) {
        let access = next_access
            .checked_add(1)
            .expect("HTTP cache access counter exhausted");
        *next_access = access;
        entry.last_access = access;
        entry.last_access_at = Self::now();
    }

    fn expired(config: CacheConfig, entry: &CacheEntry<T>) -> bool {
        config
            .ttl
            .is_some_and(|ttl| Self::now() - entry.last_access_at >= ttl.as_millis() as f64)
    }

    fn remove_persisted(config: CacheConfig, backend: &LocalStorageBackend, key: &str) {
        if matches!(config.eviction, CacheEviction::RemovePersisted) {
            let _ = backend.remove(key);
        }
    }

    fn evict_one(state: &mut CacheState<T>) {
        let key = state
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone());
        let Some(key) = key else {
            return;
        };
        state.entries.remove(&key);
        Self::remove_persisted(state.config, &state.backend, &key);
    }

    fn load_snapshot(state: &CacheState<T>, key: &str) -> Option<T> {
        let raw = match state.backend.get(key) {
            Ok(Some(raw)) => raw,
            Ok(None) | Err(_) => return None,
        };
        match (state.decode)(&raw) {
            Ok(value) => Some(value),
            Err(_) => {
                Self::remove_persisted(state.config, &state.backend, key);
                None
            }
        }
    }

    fn ensure_entry(state: &mut CacheState<T>, key: &str) -> Option<Option<T>> {
        if state.config.capacity == 0 {
            return None;
        }

        let expired = state
            .entries
            .get(key)
            .is_some_and(|entry| Self::expired(state.config, entry));
        if expired {
            state.entries.remove(key);
            Self::remove_persisted(state.config, &state.backend, key);
        }

        if let Some(entry) = state.entries.get_mut(key) {
            Self::touch(entry, &mut state.next_access);
            return Some(entry.snapshot.clone());
        }

        if state.entries.len() >= state.config.capacity {
            Self::evict_one(state);
        }

        let access = state
            .next_access
            .checked_add(1)
            .expect("HTTP cache access counter exhausted");
        state.next_access = access;
        let entry = CacheEntry {
            snapshot: Self::load_snapshot(state, key),
            generation: 0,
            last_access: access,
            last_access_at: Self::now(),
        };
        let snapshot = entry.snapshot.clone();
        state.entries.insert(key.to_string(), entry);
        Some(snapshot)
    }

    fn begin_binding(state: &mut CacheState<T>, key: &str, snapshot: Option<T>) -> CacheBinding<T> {
        let generation = state
            .next_generation
            .checked_add(1)
            .expect("HTTP cache generation exhausted");
        state.next_generation = generation;
        let entry = state
            .entries
            .get_mut(key)
            .expect("cache entry must exist before binding");
        entry.generation = generation;
        Self::touch(entry, &mut state.next_access);
        CacheBinding {
            key: key.to_string(),
            generation,
            snapshot,
        }
    }

    pub(crate) fn binding(&self, spec: &RequestSpec) -> Option<CacheBinding<T>> {
        let key = Self::key(spec);
        self.state
            .try_update(|state| {
                let snapshot = Self::ensure_entry(state, &key)?;
                Some(Self::begin_binding(state, &key, snapshot))
            })
            .ok()
            .flatten()
    }

    pub(crate) fn cached_value(&self, spec: &RequestSpec) -> Option<T> {
        let key = Self::key(spec);
        self.state
            .try_update(|state| Self::ensure_entry(state, &key).flatten())
            .ok()
            .flatten()
    }

    pub(crate) fn completion_once_for_binding(
        &self,
        scope: Scope<'scope>,
        binding: CacheBinding<T>,
    ) -> CompletionOnce<T> {
        let state = self.state;
        let key = binding.key;
        let generation = binding.generation;
        let encode = state.with(|state| state.encode.clone());
        let backend = state.with(|state| state.backend.clone());
        scope.completion_once(unwind_safe(move |value: T| {
            let active = state.try_with(|state| {
                state
                    .entries
                    .get(&key)
                    .is_some_and(|entry| entry.generation == generation)
            })?;
            if !active {
                return Ok(());
            }

            let raw = encode(&value).map_err(|error| {
                SilexError::Framework(format!("encode HTTP cache value failed: {error}"))
            })?;
            backend.set(&key, &raw).map_err(|error| {
                SilexError::Framework(format!("write HTTP cache value failed: {error}"))
            })?;

            state
                .try_update(|state| {
                    if let Some(entry) = state.entries.get_mut(&key)
                        && entry.generation == generation
                    {
                        entry.snapshot = Some(value);
                    }
                })
                .map_err(SilexError::Reactivity)?;
            Ok(())
        }))
    }
}
