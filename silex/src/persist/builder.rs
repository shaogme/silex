use crate::persist::backend::{
    LocalStorageBackend, PersistenceBackend, QueryBackend, SessionStorageBackend,
};
use crate::persist::codec::{
    map_decode_error, map_encode_error, OptionCodec, ParseCodec, PersistCodec, StringCodec,
};
use crate::persist::error::PersistenceError;
use crate::persist::policy::{DecodePolicy, PersistMode, RemovePolicy, SyncStrategy, WriteDefault};
use crate::persist::state::{
    apply_backend_event, flush_persistent_value, PersistenceController, PersistenceState,
    Persistent,
};
use silex_core::reactivity::{on_cleanup, Effect, RwSignal, StoredValue};
use silex_core::traits::{RxData, RxGet, RxRead, RxWrite};
use std::marker::PhantomData;
use std::rc::Rc;

/// Typestate marker used before a persistence backend has been selected.
pub struct NoBackend;
/// Typestate marker used before a codec has been selected.
pub struct NoCodec;

struct PersistConfig<T> {
    default: Option<Rc<dyn Fn() -> T>>,
    write_default: WriteDefault,
    decode_policy: DecodePolicy,
    remove_policy: RemovePolicy,
    mode: PersistMode,
    sync: SyncStrategy,
}

impl<T> PersistConfig<T> {
    fn new() -> Self {
        Self {
            default: None,
            write_default: WriteDefault::IfMissing,
            decode_policy: DecodePolicy::RemoveAndUseDefault,
            remove_policy: RemovePolicy::UseDefault,
            mode: PersistMode::Immediate,
            sync: SyncStrategy::CrossContext,
        }
    }
}

/// Builder for creating a `Persistent<T>` binding.
///
/// Typical flow:
/// 1. Start with [`persistent`]
/// 2. Choose a backend with `.local()`, `.session()`, or `.query()`
/// 3. Choose a codec with `.string()`, `.parse::<T>()`, or `.json::<T>()`
/// 4. Provide a default with `.default(...)` or `.default_with(...)`
/// 5. Call `.build()`
///
/// ```rust,no_run
/// use silex::prelude::*;
///
/// let theme = persistent("theme")
///     .local()
///     .string()
///     .default("Light".to_string())
///     .build();
///
/// let page = persistent("page")
///     .query()
///     .parse::<u32>()
///     .default(1)
///     .build();
///
/// theme.set("Dark".to_string());
/// assert_eq!(page.get(), 1);
/// ```
pub struct PersistentBuilder<B, C, T = ()> {
    key: String,
    backend: B,
    codec: C,
    config: PersistConfig<T>,
}

/// Starts a new persistent binding builder for the given backend key.
pub fn persistent(key: impl Into<String>) -> PersistentBuilder<NoBackend, NoCodec> {
    PersistentBuilder {
        key: key.into(),
        backend: NoBackend,
        codec: NoCodec,
        config: PersistConfig::new(),
    }
}

impl<C, T> PersistentBuilder<NoBackend, C, T> {
    /// Uses `localStorage` as the persistence backend.
    pub fn local(self) -> PersistentBuilder<LocalStorageBackend, C, T> {
        PersistentBuilder {
            key: self.key,
            backend: LocalStorageBackend,
            codec: self.codec,
            config: self.config,
        }
    }

    /// Uses `sessionStorage` as the persistence backend.
    pub fn session(self) -> PersistentBuilder<SessionStorageBackend, C, T> {
        PersistentBuilder {
            key: self.key,
            backend: SessionStorageBackend,
            codec: self.codec,
            config: self.config,
        }
    }

    /// Uses the router query string as the persistence backend.
    ///
    /// This must run inside a router context.
    pub fn query(self) -> PersistentBuilder<QueryBackend, C, T> {
        PersistentBuilder {
            key: self.key,
            backend: QueryBackend::new().unwrap_or_else(|_| QueryBackend::unavailable()),
            codec: self.codec,
            config: self.config,
        }
    }
}

impl<B, T> PersistentBuilder<B, NoCodec, T> {
    /// Uses the raw string value as-is.
    pub fn string(self) -> PersistentBuilder<B, StringCodec, String> {
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: StringCodec,
            config: PersistConfig {
                default: None,
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
        }
    }

    /// Uses `Display`/`FromStr` for encoding and decoding.
    pub fn parse<U>(self) -> PersistentBuilder<B, ParseCodec<U>, U>
    where
        U: std::fmt::Display + std::str::FromStr + Clone + 'static,
        <U as std::str::FromStr>::Err: std::fmt::Display,
    {
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: ParseCodec::new(),
            config: PersistConfig {
                default: None,
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
        }
    }

    /// Uses the JSON codec for complex serializable values.
    #[cfg(feature = "persistence")]
    pub fn json<U>(self) -> PersistentBuilder<B, crate::persist::JsonCodec<U>, U>
    where
        U: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static,
    {
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: crate::persist::JsonCodec::new(),
            config: PersistConfig {
                default: None,
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
        }
    }
}

impl<B, C, T> PersistentBuilder<B, C, T> {
    /// Configures whether a missing backend value should be initialized from the default.
    pub fn write_default(mut self, policy: WriteDefault) -> Self {
        self.config.write_default = policy;
        self
    }

    /// Configures how decode failures are handled.
    pub fn on_decode_error(mut self, policy: DecodePolicy) -> Self {
        self.config.decode_policy = policy;
        self
    }

    /// Configures how external removals are handled in memory.
    pub fn on_remove(mut self, policy: RemovePolicy) -> Self {
        self.config.remove_policy = policy;
        self
    }

    /// Selects immediate or manual flush behavior.
    pub fn mode(mut self, mode: PersistMode) -> Self {
        self.config.mode = mode;
        self
    }

    /// Selects whether external backend changes should be observed.
    pub fn sync(mut self, sync: SyncStrategy) -> Self {
        self.config.sync = sync;
        self
    }
}

impl<B, C, T> PersistentBuilder<B, C, T>
where
    T: Clone + 'static,
{
    /// Sets the default value used when the backend is empty or invalid.
    pub fn default(mut self, value: T) -> Self {
        let value = Rc::new(value);
        self.config.default = Some({
            let value = value.clone();
            Rc::new(move || (*value).clone())
        });
        self
    }

    /// Lazily computes the default value used when the backend is empty or invalid.
    pub fn default_with(mut self, f: impl Fn() -> T + 'static) -> Self {
        self.config.default = Some(Rc::new(f));
        self
    }
}

impl<B, C, T> PersistentBuilder<B, C, T>
where
    C: PersistCodec<T>,
    T: Clone + 'static,
{
    /// Promotes the binding to `Option<T>` using the selected codec for `Some(T)`.
    pub fn optional(self) -> PersistentBuilder<B, OptionCodec<C, T>, Option<T>> {
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: OptionCodec::new(self.codec),
            config: PersistConfig {
                default: Some(Rc::new(|| None)),
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
        }
    }
}

impl<B, C, T> PersistentBuilder<B, C, T>
where
    B: PersistenceBackend,
    C: PersistCodec<T>,
    T: RxData + Clone + PartialEq + 'static,
{
    /// Finalizes the builder and creates a `Persistent<T>`.
    pub fn build(self) -> Persistent<T> {
        let default = self.config.default.unwrap_or_else(|| {
            panic!(
                "persistent `{}` requires `.default(...)` or `.optional()` before `.build()`",
                self.key
            )
        });

        let value = RwSignal::new(default());
        let state = RwSignal::new(PersistenceState::Ready);

        let backend = self.backend.clone();
        let codec = self.codec.clone();
        let key = self.key.clone();

        let controller = StoredValue::new(PersistenceController {
            key: key.clone(),
            default: default.clone(),
            decode_policy: self.config.decode_policy,
            remove_policy: self.config.remove_policy,
            last_flushed_raw: None,
            skip_next_auto_flush: false,
            backend_get: Rc::new({
                let backend = backend.clone();
                move |key| backend.get(key)
            }),
            backend_set: Rc::new({
                let backend = backend.clone();
                move |key, raw| backend.set(key, raw)
            }),
            backend_remove: Rc::new({
                let backend = backend.clone();
                move |key| backend.remove(key)
            }),
            encode: Rc::new({
                let codec = codec.clone();
                move |value| codec.encode(value).map_err(map_encode_error)
            }),
            decode: Rc::new(move |raw| codec.decode(raw).map_err(|err| map_decode_error(raw, err))),
            should_remove: Rc::new({
                let codec = self.codec.clone();
                move |value| codec.should_remove(value)
            }),
            subscription: None,
        });

        let mut had_missing_value = false;

        match backend.get(&key) {
            Ok(Some(raw)) => match self.codec.decode(&raw) {
                Ok(decoded) => {
                    value.set_untracked(decoded);
                    let _ = controller.try_update_untracked(|controller| {
                        controller.last_flushed_raw = Some(raw.clone());
                    });
                    state.set_untracked(PersistenceState::Ready);
                }
                Err(message) => {
                    state.set_untracked(PersistenceState::DecodeError(
                        crate::persist::DecodeErrorInfo {
                            raw: raw.clone(),
                            message: message.clone(),
                        },
                    ));
                    value.set_untracked(default());
                    let _ = controller.try_update_untracked(|controller| {
                        controller.last_flushed_raw = None;
                    });
                    if matches!(self.config.decode_policy, DecodePolicy::RemoveAndUseDefault) {
                        let _ = backend.remove(&key);
                    }
                }
            },
            Ok(None) => {
                had_missing_value = true;
                value.set_untracked(default());
                state.set_untracked(PersistenceState::Ready);
            }
            Err(PersistenceError::BackendUnavailable) => {
                value.set_untracked(default());
                state.set_untracked(PersistenceState::Unavailable);
                let _ = controller.try_update_untracked(|controller| {
                    controller.skip_next_auto_flush = true;
                });
            }
            Err(err) => {
                value.set_untracked(default());
                state.set_untracked(PersistenceState::ReadError(err.message()));
                let _ = controller.try_update_untracked(|controller| {
                    controller.skip_next_auto_flush = true;
                });
            }
        }

        if had_missing_value {
            match self.config.write_default {
                WriteDefault::Always | WriteDefault::IfMissing => {
                    let _ = flush_persistent_value(controller, value, state);
                }
                WriteDefault::Never => {
                    let _ = controller.try_update_untracked(|controller| {
                        controller.skip_next_auto_flush = true;
                    });
                }
            }
        }

        if matches!(self.config.sync, SyncStrategy::CrossContext) {
            let callback =
                Rc::new(move |event| apply_backend_event(controller, value, state, event));
            if let Ok(subscription) = backend.subscribe(key.clone(), callback) {
                let _ = controller.try_update_untracked(|controller| {
                    controller.subscription = Some(subscription);
                });
            }
        }

        if matches!(self.config.mode, PersistMode::Immediate) {
            Effect::new(move |_| {
                value.get();
                let should_skip =
                    controller.with_untracked(|controller| controller.skip_next_auto_flush);
                if should_skip {
                    let _ = controller.try_update_untracked(|controller| {
                        controller.skip_next_auto_flush = false;
                    });
                    return;
                }
                let _ = flush_persistent_value(controller, value, state);
            });
        }

        on_cleanup(move || {
            let _ = controller.try_update_untracked(|controller| {
                controller.subscription.take();
            });
        });

        Persistent {
            value,
            state,
            controller,
        }
    }
}

impl<B, C, T> PersistentBuilder<B, C, T> {
    #[allow(dead_code)]
    fn _marker(&self) -> PhantomData<T> {
        PhantomData
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::backend::{BackendEvent, BackendSubscription};
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Clone, Default)]
    struct MockBackend {
        state: Rc<RefCell<HashMap<String, String>>>,
        removed: Rc<RefCell<Vec<String>>>,
        subscriptions: Rc<RefCell<HashMap<String, Vec<Rc<dyn Fn(BackendEvent)>>>>>,
        fail_writes: Rc<RefCell<bool>>,
    }

    impl MockBackend {
        fn with_value(key: &str, value: &str) -> Self {
            let mut state = HashMap::new();
            state.insert(key.to_string(), value.to_string());
            Self {
                state: Rc::new(RefCell::new(state)),
                removed: Rc::new(RefCell::new(Vec::new())),
                subscriptions: Rc::new(RefCell::new(HashMap::new())),
                fail_writes: Rc::new(RefCell::new(false)),
            }
        }

        fn failing_writes() -> Self {
            Self {
                state: Rc::new(RefCell::new(HashMap::new())),
                removed: Rc::new(RefCell::new(Vec::new())),
                subscriptions: Rc::new(RefCell::new(HashMap::new())),
                fail_writes: Rc::new(RefCell::new(true)),
            }
        }

        fn emit(&self, key: &str, event: BackendEvent) {
            let callbacks = self
                .subscriptions
                .borrow()
                .get(key)
                .cloned()
                .unwrap_or_default();
            for callback in callbacks {
                callback(event.clone());
            }
        }
    }

    impl PersistenceBackend for MockBackend {
        fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
            Ok(self.state.borrow().get(key).cloned())
        }

        fn set(&self, key: &str, value: &str) -> Result<(), PersistenceError> {
            if *self.fail_writes.borrow() {
                return Err(PersistenceError::WriteFailed(
                    "mock backend write failure".to_string(),
                ));
            }
            self.state
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn remove(&self, key: &str) -> Result<(), PersistenceError> {
            self.state.borrow_mut().remove(key);
            self.removed.borrow_mut().push(key.to_string());
            Ok(())
        }

        fn subscribe(
            &self,
            key: String,
            callback: Rc<dyn Fn(BackendEvent)>,
        ) -> Result<BackendSubscription, PersistenceError> {
            self.subscriptions
                .borrow_mut()
                .entry(key)
                .or_default()
                .push(callback);
            Ok(BackendSubscription::new(|| {}))
        }
    }

    fn parse_builder(
        backend: MockBackend,
        key: &str,
    ) -> PersistentBuilder<MockBackend, ParseCodec<i32>, i32> {
        PersistentBuilder {
            key: key.to_string(),
            backend,
            codec: ParseCodec::new(),
            config: PersistConfig::new(),
        }
    }

    #[test]
    fn write_default_if_missing_persists_default() {
        let backend = MockBackend::default();
        let value = parse_builder(backend.clone(), "counter").default(7).build();

        assert_eq!(value.get_untracked(), 7);
        assert_eq!(backend.get("counter").unwrap(), Some("7".to_string()));
        assert_eq!(value.state().get_untracked(), PersistenceState::Ready);
    }

    #[test]
    fn decode_error_remove_and_use_default_removes_invalid_backend_value() {
        let backend = MockBackend::with_value("counter", "bad");
        let value = parse_builder(backend.clone(), "counter")
            .on_decode_error(DecodePolicy::RemoveAndUseDefault)
            .default(5)
            .build();

        assert_eq!(value.get_untracked(), 5);
        assert_eq!(backend.get("counter").unwrap(), Some("5".to_string()));
        assert_eq!(
            backend.removed.borrow().as_slice(),
            &["counter".to_string()]
        );
        assert_eq!(value.state().get_untracked(), PersistenceState::Ready);
    }

    #[test]
    fn decode_error_use_default_keeps_invalid_backend_value() {
        let backend = MockBackend::with_value("counter", "bad");
        let value = parse_builder(backend.clone(), "counter")
            .on_decode_error(DecodePolicy::UseDefault)
            .default(11)
            .build();

        assert_eq!(value.get_untracked(), 11);
        assert_eq!(backend.get("counter").unwrap(), Some("11".to_string()));
        assert!(backend.removed.borrow().is_empty());
        assert_eq!(value.state().get_untracked(), PersistenceState::Ready);
    }

    #[test]
    fn codec_selection_preserves_builder_configuration() {
        let backend = MockBackend::default();
        let value = PersistentBuilder {
            key: "counter".to_string(),
            backend,
            codec: NoCodec,
            config: PersistConfig::<()>::new(),
        }
        .mode(PersistMode::Manual)
        .sync(SyncStrategy::None)
        .write_default(WriteDefault::Never)
        .parse::<i32>()
        .default(9)
        .build();

        assert_eq!(value.get_untracked(), 9);
        assert_eq!(value.state().get_untracked(), PersistenceState::Ready);
    }

    #[test]
    fn flush_write_failure_sets_write_error_state() {
        let backend = MockBackend::failing_writes();
        let value = parse_builder(backend, "counter").default(3).build();

        assert!(matches!(
            value.flush(),
            Err(PersistenceError::WriteFailed(message)) if message == "mock backend write failure"
        ));
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock backend write failure".to_string())
        );
    }

    #[test]
    fn initial_default_write_failure_sets_write_error_state() {
        let backend = MockBackend::failing_writes();
        let value = parse_builder(backend, "counter").default(3).build();

        assert_eq!(value.get_untracked(), 3);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::WriteError("mock backend write failure".to_string())
        );
    }

    #[test]
    fn optional_none_flush_removes_backend_key() {
        let backend = MockBackend::with_value("name", "alice");
        let value = PersistentBuilder {
            key: "name".to_string(),
            backend: backend.clone(),
            codec: StringCodec,
            config: PersistConfig::<String>::new(),
        }
        .optional()
        .build();

        assert_eq!(value.get_untracked(), Some("alice".to_string()));

        value.set(None);
        value.flush().unwrap();

        assert_eq!(backend.get("name").unwrap(), None);
        assert_eq!(backend.removed.borrow().as_slice(), &["name".to_string()]);
        assert_eq!(value.state().get_untracked(), PersistenceState::Ready);
    }

    #[test]
    fn external_remove_uses_default_without_rewriting_backend() {
        let backend = MockBackend::with_value("counter", "7");
        let value = parse_builder(backend.clone(), "counter").default(5).build();

        assert_eq!(value.get_untracked(), 7);

        backend.state.borrow_mut().remove("counter");
        backend.emit(
            "counter",
            BackendEvent::Removed {
                key: "counter".to_string(),
            },
        );

        assert_eq!(value.get_untracked(), 5);
        assert_eq!(backend.get("counter").unwrap(), None);
        assert!(backend.removed.borrow().is_empty());
        assert_eq!(value.state().get_untracked(), PersistenceState::Ready);
    }
}
