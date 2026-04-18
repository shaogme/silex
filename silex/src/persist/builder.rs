use crate::persist::backend::{
    LocalStorageBackend, PersistenceBackend, QueryBackend, SessionStorageBackend,
};
use crate::persist::codec::{
    OptionCodec, ParseCodec, PersistCodec, StringCodec, map_decode_error, map_encode_error,
};
use crate::persist::state::{
    PersistenceController, PersistenceState, Persistent, apply_backend_event,
    flush_persistent_value,
};
use crate::persist::{
    DecodePolicy, PersistMode, PersistenceError, RemovePolicy, SyncStrategy, WriteDefault,
};
use silex_core::reactivity::{Effect, RwSignal, StoredValue, on_cleanup};
use silex_core::traits::{RxData, RxGet, RxRead, RxWrite};
use std::marker::PhantomData;
use std::rc::Rc;

/// Typestate marker used before a persistence backend has been selected.
pub struct NoBackend;
/// Typestate marker used before a codec has been selected.
pub struct NoCodec;
/// Typestate marker used before a default value has been selected.
pub struct NoDefault;
/// Typestate marker indicating a default value has been selected.
pub struct HasDefault;

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
/// 1. Start with [`Persistent::builder`]
/// 2. Choose a backend with `.local()`, `.session()`, or `.query()`
/// 3. Choose a codec with `.string()`, `.parse::<T>()`, or `.json::<T>()`
/// 4. Provide a default with `.default(...)` or `.default_with(...)`
/// 5. Call `.build()`
///
/// ```rust,no_run
/// use silex::prelude::*;
///
/// let theme = Persistent::builder("theme")
///     .local()
///     .string()
///     .default("Light".to_string())
///     .build();
///
/// let page = Persistent::builder("page")
///     .query()
///     .parse::<u32>()
///     .default(1)
///     .build();
///
/// theme.set("Dark".to_string());
/// assert_eq!(page.get(), 1);
/// ```
pub struct PersistentBuilder<B, C, T = (), D = NoDefault> {
    key: String,
    backend: B,
    codec: C,
    config: PersistConfig<T>,
    _marker: PhantomData<D>,
}

impl PersistentBuilder<NoBackend, NoCodec, (), NoDefault> {
    /// Starts a new persistent binding builder for the given backend key.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            backend: NoBackend,
            codec: NoCodec,
            config: PersistConfig::new(),
            _marker: PhantomData,
        }
    }
}

impl<C, T, D> PersistentBuilder<NoBackend, C, T, D> {
    /// Uses `localStorage` as the persistence backend.
    pub fn local(self) -> PersistentBuilder<LocalStorageBackend, C, T, D> {
        PersistentBuilder {
            key: self.key,
            backend: LocalStorageBackend::default(),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    /// Uses `sessionStorage` as the persistence backend.
    pub fn session(self) -> PersistentBuilder<SessionStorageBackend, C, T, D> {
        PersistentBuilder {
            key: self.key,
            backend: SessionStorageBackend::default(),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    /// Uses the router query string as the persistence backend.
    ///
    /// This must run inside a router context.
    pub fn query(self) -> PersistentBuilder<QueryBackend, C, T, D> {
        PersistentBuilder {
            key: self.key,
            backend: QueryBackend::new().unwrap_or_else(|_| QueryBackend::unavailable()),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }
}

impl<B, T, D> PersistentBuilder<B, NoCodec, T, D> {
    /// Uses the raw string value as-is.
    pub fn string(self) -> PersistentBuilder<B, StringCodec, String, D> {
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
            _marker: PhantomData,
        }
    }

    /// Uses `Display`/`FromStr` for encoding and decoding.
    pub fn parse<U>(self) -> PersistentBuilder<B, ParseCodec<U>, U, D>
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
            _marker: PhantomData,
        }
    }

    /// Uses the JSON codec for complex serializable values.
    #[cfg(feature = "json")]
    pub fn json<U>(self) -> PersistentBuilder<B, crate::persist::PersistJsonCodec<U>, U, D>
    where
        U: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static,
    {
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: crate::persist::PersistJsonCodec::new(),
            config: PersistConfig {
                default: None,
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
            _marker: PhantomData,
        }
    }
}

impl<B, C, T, D> PersistentBuilder<B, C, T, D> {
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

impl<B, C, T, D> PersistentBuilder<B, C, T, D>
where
    T: Clone + 'static,
{
    /// Sets the default value used when the backend is empty or invalid.
    pub fn default(self, value: T) -> PersistentBuilder<B, C, T, HasDefault> {
        let value = Rc::new(value);
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: self.codec,
            config: PersistConfig {
                default: Some({
                    let value = value.clone();
                    Rc::new(move || (*value).clone())
                }),
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
            _marker: PhantomData,
        }
    }

    /// Lazily computes the default value used when the backend is empty or invalid.
    pub fn default_with(
        self,
        f: impl Fn() -> T + 'static,
    ) -> PersistentBuilder<B, C, T, HasDefault> {
        PersistentBuilder {
            key: self.key,
            backend: self.backend,
            codec: self.codec,
            config: PersistConfig {
                default: Some(Rc::new(f)),
                write_default: self.config.write_default,
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                mode: self.config.mode,
                sync: self.config.sync,
            },
            _marker: PhantomData,
        }
    }
}

impl<B, C, T, D> PersistentBuilder<B, C, T, D>
where
    C: PersistCodec<T>,
    T: Clone + 'static,
{
    /// Promotes the binding to `Option<T>` using the selected codec for `Some(T)`.
    pub fn optional(self) -> PersistentBuilder<B, OptionCodec<C, T>, Option<T>, HasDefault> {
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
            _marker: PhantomData,
        }
    }
}

impl<B, C, T> PersistentBuilder<B, C, T, HasDefault>
where
    B: PersistenceBackend,
    C: PersistCodec<T>,
    T: RxData + Clone + PartialEq + 'static,
{
    /// Finalizes the builder and creates a `Persistent<T>`.
    pub fn build(self) -> Persistent<T> {
        let default = self
            .config
            .default
            .expect("Default status verified by typestate");

        let value = RwSignal::new(default());
        let state = RwSignal::new(PersistenceState::Ready(String::new()));

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
                    state.set_untracked(PersistenceState::Ready(raw));
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
                state.set_untracked(PersistenceState::Ready(String::new()));
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
            let debounce_duration = match self.config.sync {
                SyncStrategy::Debounce(d) => Some(d),
                _ => None,
            };

            if let Some(duration) = debounce_duration {
                let timer = StoredValue::new(None::<silex_dom::helpers::TimeoutHandle>);
                Effect::new(move |_| {
                    let current = value.get();
                    let should_skip =
                        controller.with_untracked(|controller| controller.skip_next_auto_flush);
                    if should_skip {
                        let _ = controller.try_update_untracked(|controller| {
                            controller.skip_next_auto_flush = false;
                        });
                        return;
                    }

                    // Update state to Syncing with current raw value
                    let raw = controller.with_untracked(|c| (c.encode)(&current));
                    if let Ok(raw) = raw {
                        state.set(PersistenceState::Syncing(raw));
                    }

                    if let Some(handle) = timer.get_untracked() {
                        handle.clear();
                    }

                    let handle = silex_dom::helpers::set_timeout_with_handle(
                        move || {
                            let _ = flush_persistent_value(controller, value, state);
                            timer.set_untracked(None);
                        },
                        duration,
                    );

                    if let Ok(h) = handle {
                        timer.set_untracked(Some(h));
                    }
                });

                on_cleanup(move || {
                    if let Some(handle) = timer.get_untracked() {
                        handle.clear();
                    }
                });
            } else {
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

impl<B, C, T, D> PersistentBuilder<B, C, T, D> {
    fn _marker(&self) -> PhantomData<(T, D)> {
        PhantomData
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::backend::{BackendEvent, BackendSubscription};
    use std::cell::RefCell;
    use std::collections::HashMap;

    type SubscriptionMap = Rc<RefCell<HashMap<String, Vec<Rc<dyn Fn(BackendEvent)>>>>>;

    #[derive(Clone, Default)]
    struct MockBackend {
        state: Rc<RefCell<HashMap<String, String>>>,
        removed: Rc<RefCell<Vec<String>>>,
        subscriptions: SubscriptionMap,
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
    ) -> PersistentBuilder<MockBackend, ParseCodec<i32>, i32, NoDefault> {
        PersistentBuilder {
            key: key.to_string(),
            backend,
            codec: ParseCodec::new(),
            config: PersistConfig::new(),
            _marker: PhantomData,
        }
    }

    #[test]
    fn write_default_if_missing_persists_default() {
        let backend = MockBackend::default();
        let value = parse_builder(backend.clone(), "counter").default(7).build();

        assert_eq!(value.get_untracked(), 7);
        assert_eq!(backend.get("counter").unwrap(), Some("7".to_string()));
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("7".to_string())
        );
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
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("5".to_string())
        );
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
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("11".to_string())
        );
    }

    #[test]
    fn codec_selection_preserves_builder_configuration() {
        let backend = MockBackend::default();
        let value = PersistentBuilder {
            key: "counter".to_string(),
            backend,
            codec: NoCodec,
            config: PersistConfig::<()>::new(),
            _marker: PhantomData::<NoDefault>,
        }
        .mode(PersistMode::Manual)
        .sync(SyncStrategy::None)
        .write_default(WriteDefault::Never)
        .parse::<i32>()
        .default(9)
        .build();

        assert_eq!(value.get_untracked(), 9);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready("9".to_string())
        );
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
            _marker: PhantomData::<NoDefault>,
        }
        .optional()
        .build();

        assert_eq!(value.get_untracked(), Some("alice".to_string()));

        value.set(None);
        value.flush().unwrap();

        assert_eq!(backend.get("name").unwrap(), None);
        assert_eq!(backend.removed.borrow().as_slice(), &["name".to_string()]);
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
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
        assert_eq!(
            value.state().get_untracked(),
            PersistenceState::Ready(String::new())
        );
    }
}
