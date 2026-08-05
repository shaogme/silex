use crate::{
    DecodePolicy, PersistMode, PersistenceError, RemovePolicy, SyncStrategy, WriteDefault,
    backend::{
        BackendEventSink, LocalStorageBackend, PersistenceBackend, QueryBackend,
        SessionStorageBackend,
    },
    codec::{
        OptionCodec, ParseCodec, PersistCodec, StringCodec, map_decode_error, map_encode_error,
    },
    state::{
        PersistenceController, PersistenceState, Persistent, ScopedDebounceState,
        apply_backend_event, flush_persistent_value,
    },
};
use ref_str::LocalStaticRefStr;
use silex_core::{
    RxBase, RxRead, Scope,
    traits::{RxGet, RxWrite},
};
use silex_dom::helpers::set_timeout_with_handle;
use silex_router::RouterContext;
use std::{borrow::Cow, cell::RefCell, marker::PhantomData, rc::Rc};

/// Typestate marker used before a persistence backend has been selected.
pub struct NoBackend;
/// Typestate marker used before a codec has been selected.
pub struct NoCodec;
/// Typestate marker used before a default value has been selected.
pub struct NoDefault;
/// Typestate marker indicating a default value has been selected.
pub struct HasDefault;

struct PersistConfig<'scope, T: 'scope> {
    default: Option<Rc<dyn Fn() -> T + 'scope>>,
    write_default: WriteDefault,
    decode_policy: DecodePolicy,
    remove_policy: RemovePolicy,
    mode: PersistMode,
    sync: SyncStrategy,
}

impl<'scope, T: 'scope> PersistConfig<'scope, T> {
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

/// Builder for creating a scoped `Persistent<'scope, T>` binding.
pub struct PersistentBuilder<'scope, B, C, T = (), D = NoDefault>
where
    T: 'scope,
{
    scope: Scope<'scope>,
    key: LocalStaticRefStr,
    backend: B,
    codec: C,
    config: PersistConfig<'scope, T>,
    _marker: PhantomData<D>,
}

impl<'scope> PersistentBuilder<'scope, NoBackend, NoCodec, (), NoDefault> {
    pub fn new(scope: Scope<'scope>, key: impl Into<LocalStaticRefStr>) -> Self {
        Self {
            scope,
            key: key.into(),
            backend: NoBackend,
            codec: NoCodec,
            config: PersistConfig::new(),
            _marker: PhantomData,
        }
    }
}

impl<'scope, C, T, D> PersistentBuilder<'scope, NoBackend, C, T, D>
where
    T: 'scope,
{
    pub fn local(self) -> PersistentBuilder<'scope, LocalStorageBackend, C, T, D> {
        PersistentBuilder {
            scope: self.scope,
            key: self.key,
            backend: LocalStorageBackend::default(),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    pub fn session(self) -> PersistentBuilder<'scope, SessionStorageBackend, C, T, D> {
        PersistentBuilder {
            scope: self.scope,
            key: self.key,
            backend: SessionStorageBackend::default(),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    pub fn query(
        self,
        ctx: &RouterContext<'scope>,
    ) -> PersistentBuilder<'scope, QueryBackend<'scope>, C, T, D> {
        PersistentBuilder {
            scope: self.scope,
            key: self.key,
            backend: QueryBackend::new(ctx),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }
}

impl<'scope, B, T, D> PersistentBuilder<'scope, B, NoCodec, T, D>
where
    T: 'scope,
{
    pub fn string(self) -> PersistentBuilder<'scope, B, StringCodec, String, D> {
        PersistentBuilder {
            scope: self.scope,
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

    pub fn cow(self) -> PersistentBuilder<'scope, B, StringCodec, Cow<'static, str>, D> {
        PersistentBuilder {
            scope: self.scope,
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

    pub fn parse<U>(self) -> PersistentBuilder<'scope, B, ParseCodec<U>, U, D>
    where
        U: std::fmt::Display + std::str::FromStr + Clone + 'scope,
        <U as std::str::FromStr>::Err: std::fmt::Display,
    {
        PersistentBuilder {
            scope: self.scope,
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

    #[cfg(feature = "json")]
    pub fn json<U>(self) -> PersistentBuilder<'scope, B, crate::PersistJsonCodec<U>, U, D>
    where
        U: serde::Serialize + serde::de::DeserializeOwned + Clone + 'scope,
    {
        PersistentBuilder {
            scope: self.scope,
            key: self.key,
            backend: self.backend,
            codec: crate::PersistJsonCodec::new(),
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

impl<'scope, B, C, T, D> PersistentBuilder<'scope, B, C, T, D>
where
    T: 'scope,
{
    pub fn write_default(mut self, policy: WriteDefault) -> Self {
        self.config.write_default = policy;
        self
    }

    pub fn on_decode_error(mut self, policy: DecodePolicy) -> Self {
        self.config.decode_policy = policy;
        self
    }

    pub fn on_remove(mut self, policy: RemovePolicy) -> Self {
        self.config.remove_policy = policy;
        self
    }

    pub fn mode(mut self, mode: PersistMode) -> Self {
        self.config.mode = mode;
        self
    }

    pub fn sync(mut self, sync: SyncStrategy) -> Self {
        self.config.sync = sync;
        self
    }
}

impl<'scope, B, C, T, D> PersistentBuilder<'scope, B, C, T, D>
where
    T: Clone + 'scope,
{
    pub fn default(self, value: T) -> PersistentBuilder<'scope, B, C, T, HasDefault> {
        let value = Rc::new(value);
        PersistentBuilder {
            scope: self.scope,
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

    pub fn default_with(
        self,
        f: impl Fn() -> T + 'scope,
    ) -> PersistentBuilder<'scope, B, C, T, HasDefault> {
        PersistentBuilder {
            scope: self.scope,
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

impl<'scope, B, C, T, D> PersistentBuilder<'scope, B, C, T, D>
where
    C: PersistCodec<T> + 'scope,
    T: Clone + 'scope,
{
    pub fn optional(
        self,
    ) -> PersistentBuilder<'scope, B, OptionCodec<C, T>, Option<T>, HasDefault> {
        PersistentBuilder {
            scope: self.scope,
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

impl<'scope, B, C, T> PersistentBuilder<'scope, B, C, T, HasDefault>
where
    B: PersistenceBackend<'scope>,
    C: PersistCodec<T> + 'scope,
    T: Clone + PartialEq + 'scope,
{
    pub fn try_build(self) -> Result<Persistent<'scope, T>, PersistenceError> {
        let inputs = self.backend.runtime_inputs();
        self.scope
            .try_validate_inputs(&inputs)
            .map_err(|error| PersistenceError::InvalidConfiguration(error.to_string()))?;

        let default = self
            .config
            .default
            .expect("Default status verified by typestate");
        let value = self.scope.rw_signal(default());
        let state = self.scope.rw_signal(PersistenceState::Ready(String::new()));
        let backend = self.backend.clone();
        let codec = self.codec.clone();
        let key = self.key.clone();
        let subscription = Rc::new(RefCell::new(None));
        let debounce = if matches!(self.config.mode, PersistMode::Immediate)
            && let SyncStrategy::Debounce(_) = self.config.sync
        {
            Some(Rc::new(RefCell::new(ScopedDebounceState::new())))
        } else {
            None
        };
        let controller = self.scope.stored(PersistenceController {
            key: key.clone(),
            default: default.clone(),
            decode_policy: self.config.decode_policy,
            remove_policy: self.config.remove_policy,
            last_flushed_raw: None,
            skip_next_auto_flush: false,
            suppress_manual_state: false,
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
            decode: Rc::new({
                let codec = codec.clone();
                move |raw| {
                    codec
                        .decode(raw)
                        .map_err(|error| map_decode_error(raw, error))
                }
            }),
            should_remove: Rc::new({
                let codec = self.codec.clone();
                move |value| codec.should_remove(value)
            }),
            subscription: subscription.clone(),
            debounce: debounce.clone(),
        });
        let subscription = controller.with_untracked(|controller| controller.subscription.clone());

        let mut had_missing_value = false;
        let mut initial_error = false;
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
                    state.set_untracked(PersistenceState::DecodeError(crate::DecodeErrorInfo {
                        raw: raw.clone(),
                        message: message.clone(),
                    }));
                    value.set_untracked(default());
                    let _ = controller.try_update_untracked(|controller| {
                        controller.last_flushed_raw = None;
                        controller.skip_next_auto_flush = true;
                        controller.suppress_manual_state = true;
                    });
                    initial_error = true;
                    if matches!(self.config.decode_policy, DecodePolicy::RemoveAndUseDefault)
                        && let Err(error) = backend.remove(&key)
                    {
                        state.set_untracked(PersistenceState::WriteError(error.message()));
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
                    controller.suppress_manual_state = true;
                });
                initial_error = true;
            }
            Err(error) => {
                value.set_untracked(default());
                state.set_untracked(PersistenceState::ReadError(error.message()));
                let _ = controller.try_update_untracked(|controller| {
                    controller.skip_next_auto_flush = true;
                    controller.suppress_manual_state = true;
                });
                initial_error = true;
            }
        }

        if had_missing_value {
            match self.config.write_default {
                WriteDefault::Never => {
                    let _ = controller.try_update_untracked(|controller| {
                        controller.skip_next_auto_flush = true;
                    });
                }
                WriteDefault::IfMissing | WriteDefault::Always => {
                    if flush_persistent_value(controller, value, state).is_err() {
                        initial_error = true;
                        let _ = controller.try_update_untracked(|controller| {
                            controller.skip_next_auto_flush = true;
                            controller.suppress_manual_state = true;
                        });
                    }
                }
            }
        } else if matches!(self.config.write_default, WriteDefault::Always)
            && matches!(state.get_untracked(), PersistenceState::Ready(_))
            && flush_persistent_value(controller, value, state).is_err()
        {
            initial_error = true;
            let _ = controller.try_update_untracked(|controller| {
                controller.skip_next_auto_flush = true;
                controller.suppress_manual_state = true;
            });
        }

        if initial_error {
            let _ = controller.try_update_untracked(|controller| {
                controller.suppress_manual_state = true;
            });
        }

        if matches!(self.config.sync, SyncStrategy::CrossContext) {
            let token = self.scope.completion({
                move |event| {
                    if value.is_alive() {
                        apply_backend_event(controller, value, state, event);
                    }
                }
            });
            let sink: BackendEventSink = Rc::new(move |event| {
                let _ = token.submit(event);
            });
            match backend.subscribe(self.scope, key.clone(), sink) {
                Ok(binding) => *subscription.borrow_mut() = Some(binding),
                Err(PersistenceError::BackendUnavailable) => {}
                Err(PersistenceError::InvalidConfiguration(message)) => {
                    return Err(PersistenceError::InvalidConfiguration(message));
                }
                Err(error) => state.set_untracked(PersistenceState::WriteError(error.message())),
            }
        }

        let subscription_for_cleanup = subscription.clone();
        self.scope.on_cleanup(move || {
            let _ = subscription_for_cleanup.borrow_mut().take();
        });

        match self.config.mode {
            PersistMode::Immediate => {
                if let SyncStrategy::Debounce(duration) = self.config.sync {
                    let debounce_state = debounce
                        .clone()
                        .expect("debounce state must exist for debounce mode");
                    let debounce_for_completion = debounce_state.clone();
                    let completion = self.scope.completion({
                        move |generation| {
                            if debounce_for_completion.borrow_mut().take_ready(generation)
                                && value.is_alive()
                            {
                                let _ = flush_persistent_value(controller, value, state);
                            }
                        }
                    });
                    let debounce_for_cleanup = debounce_state.clone();
                    self.scope.on_cleanup(move || {
                        debounce_for_cleanup.borrow_mut().invalidate();
                    });
                    self.scope.effect(move || {
                        let current = value.get();
                        let should_skip =
                            controller.with_untracked(|controller| controller.skip_next_auto_flush);
                        if should_skip {
                            let _ = controller.try_update_untracked(|controller| {
                                controller.skip_next_auto_flush = false;
                            });
                            return;
                        }

                        let raw =
                            controller.with_untracked(|controller| (controller.encode)(&current));
                        let raw = match raw {
                            Ok(raw) => raw,
                            Err(error) => {
                                debounce_state.borrow_mut().invalidate();
                                state.set(PersistenceState::WriteError(error.message()));
                                return;
                            }
                        };
                        state.set(PersistenceState::Syncing(raw));
                        let generation = debounce_state.borrow_mut().begin();
                        let completion = completion.clone();
                        match set_timeout_with_handle(
                            move || {
                                let _ = completion.submit(generation);
                            },
                            duration,
                        ) {
                            Ok(timer) => {
                                let _ = debounce_state.borrow_mut().set_timer(generation, timer);
                            }
                            Err(error) => {
                                debounce_state.borrow_mut().invalidate();
                                state.set(PersistenceState::WriteError(format!(
                                    "schedule persistence timeout failed: {:?}",
                                    error
                                )));
                            }
                        }
                    });
                } else {
                    self.scope.effect(move || {
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
            PersistMode::Manual => {
                self.scope.effect(move || {
                    let current = value.get();
                    let suppress =
                        controller.with_untracked(|controller| controller.suppress_manual_state);
                    if suppress {
                        let _ = controller.try_update_untracked(|controller| {
                            controller.suppress_manual_state = false;
                        });
                        return;
                    }

                    let (raw, last_raw, is_default) = controller.with_untracked(|controller| {
                        (
                            (controller.encode)(&current),
                            controller.last_flushed_raw.clone(),
                            current == (controller.default)(),
                        )
                    });
                    if let Ok(raw) = raw {
                        let is_ready = match &last_raw {
                            Some(last) => last == &raw,
                            None => is_default,
                        };
                        if is_ready {
                            if last_raw.is_none() {
                                state.set(PersistenceState::Ready(String::new()));
                            } else {
                                state.set(PersistenceState::Ready(raw));
                            }
                        } else {
                            state.set(PersistenceState::Dirty(raw));
                        }
                    }
                });
            }
        }

        Ok(Persistent {
            value,
            state,
            controller,
        })
    }

    pub fn build(self) -> Persistent<'scope, T> {
        self.try_build()
            .unwrap_or_else(|error| panic!("persistent binding creation failed: {error:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendEvent, backend::BackendSubscription};
    use std::collections::HashMap;

    type SubscriptionMap = Rc<RefCell<HashMap<LocalStaticRefStr, Vec<(usize, BackendEventSink)>>>>;

    #[derive(Clone, Default)]
    struct MockBackend {
        state: Rc<RefCell<HashMap<String, String>>>,
        removed: Rc<RefCell<Vec<String>>>,
        writes: Rc<RefCell<Vec<(String, String)>>>,
        subscriptions: SubscriptionMap,
        next_id: Rc<std::cell::Cell<usize>>,
        fail_writes: Rc<RefCell<bool>>,
        fail_removes: Rc<RefCell<bool>>,
    }

    impl MockBackend {
        fn with_value(key: &str, value: &str) -> Self {
            let mut state = HashMap::new();
            state.insert(key.to_string(), value.to_string());
            Self {
                state: Rc::new(RefCell::new(state)),
                ..Self::default()
            }
        }

        fn failing_writes() -> Self {
            Self {
                fail_writes: Rc::new(RefCell::new(true)),
                ..Self::default()
            }
        }

        fn failing_removes() -> Self {
            Self {
                fail_removes: Rc::new(RefCell::new(true)),
                ..Self::default()
            }
        }

        fn emit(&self, key: &str, event: BackendEvent) {
            let callbacks = self
                .subscriptions
                .borrow()
                .get(key)
                .map(|subscribers| {
                    subscribers
                        .iter()
                        .map(|(_, sink)| sink.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for callback in callbacks {
                callback(event.clone());
            }
        }
    }

    impl<'scope> PersistenceBackend<'scope> for MockBackend {
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
            self.writes
                .borrow_mut()
                .push((key.to_string(), value.to_string()));
            Ok(())
        }

        fn remove(&self, key: &str) -> Result<(), PersistenceError> {
            if *self.fail_removes.borrow() {
                return Err(PersistenceError::RemoveFailed(
                    "mock backend remove failure".to_string(),
                ));
            }
            self.state.borrow_mut().remove(key);
            self.removed.borrow_mut().push(key.to_string());
            Ok(())
        }

        fn subscribe(
            &self,
            _scope: Scope<'scope>,
            key: impl Into<LocalStaticRefStr>,
            sink: BackendEventSink,
        ) -> Result<BackendSubscription<'scope>, PersistenceError> {
            let key = key.into();
            let id = self.next_id.get();
            self.next_id.set(id + 1);
            self.subscriptions
                .borrow_mut()
                .entry(key.clone())
                .or_default()
                .push((id, sink));
            let subscriptions = self.subscriptions.clone();
            Ok(BackendSubscription::new(move || {
                let mut subscriptions = subscriptions.borrow_mut();
                if let Some(subscribers) = subscriptions.get_mut(&key) {
                    subscribers.retain(|(subscriber_id, _)| *subscriber_id != id);
                    if subscribers.is_empty() {
                        subscriptions.remove(&key);
                    }
                }
            }))
        }
    }

    fn parse_builder<'scope>(
        scope: Scope<'scope>,
        backend: MockBackend,
        key: &str,
    ) -> PersistentBuilder<'scope, MockBackend, ParseCodec<i32>, i32, NoDefault> {
        PersistentBuilder {
            scope,
            key: key.to_string().into(),
            backend,
            codec: ParseCodec::new(),
            config: PersistConfig::new(),
            _marker: PhantomData,
        }
    }

    #[test]
    fn write_default_if_missing_persists_default() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::default();
            let value = parse_builder(scope, backend.clone(), "counter")
                .default(7)
                .build();
            assert_eq!(value.get_untracked(), 7);
            assert_eq!(backend.get("counter").unwrap(), Some("7".to_string()));
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::Ready("7".to_string())
            );
        });
    }

    #[test]
    fn decode_error_remove_and_use_default_keeps_decode_error_state() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::with_value("counter", "bad");
            let value = parse_builder(scope, backend.clone(), "counter")
                .on_decode_error(DecodePolicy::RemoveAndUseDefault)
                .default(5)
                .build();
            assert_eq!(value.get_untracked(), 5);
            assert_eq!(backend.get("counter").unwrap(), None);
            assert_eq!(
                backend.removed.borrow().as_slice(),
                &["counter".to_string()]
            );
            assert!(matches!(
                value.state().get_untracked(),
                PersistenceState::DecodeError(_)
            ));
        });
    }

    #[test]
    fn decode_error_use_default_preserves_invalid_raw() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::with_value("counter", "bad");
            let value = parse_builder(scope, backend.clone(), "counter")
                .on_decode_error(DecodePolicy::UseDefault)
                .default(11)
                .build();
            assert_eq!(value.get_untracked(), 11);
            assert_eq!(backend.get("counter").unwrap(), Some("bad".to_string()));
            assert!(matches!(
                value.state().get_untracked(),
                PersistenceState::DecodeError(_)
            ));
        });
    }

    #[test]
    fn write_default_always_normalizes_existing_raw() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::with_value("counter", "007");
            let _value = parse_builder(scope, backend.clone(), "counter")
                .write_default(WriteDefault::Always)
                .default(5)
                .build();
            assert_eq!(backend.get("counter").unwrap(), Some("7".to_string()));
            assert_eq!(
                backend.writes.borrow().as_slice(),
                &[("counter".to_string(), "7".to_string())]
            );
        });
    }

    #[test]
    fn initial_default_write_failure_is_visible() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::failing_writes();
            let value = parse_builder(scope, backend, "counter").default(3).build();
            assert_eq!(value.get_untracked(), 3);
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::WriteError("mock backend write failure".to_string())
            );
        });
    }

    #[test]
    fn optional_none_flush_removes_backend_key() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::with_value("name", "alice");
            let value = PersistentBuilder {
                scope,
                key: "name".into(),
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
        });
    }

    #[test]
    fn external_remove_uses_default_without_rewriting_backend() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::with_value("counter", "7");
            let value = parse_builder(scope, backend.clone(), "counter")
                .default(5)
                .build();
            backend.state.borrow_mut().remove("counter");
            backend.emit(
                "counter",
                BackendEvent::Removed {
                    key: "counter".into(),
                },
            );
            assert_eq!(value.get_untracked(), 5);
            assert_eq!(backend.get("counter").unwrap(), None);
            assert!(backend.removed.borrow().is_empty());
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::Ready(String::new())
            );
        });
    }

    #[test]
    fn subscription_is_removed_when_scope_is_disposed() {
        let backend = MockBackend::with_value("counter", "7");
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let _value = parse_builder(scope, backend.clone(), "counter")
                .default(5)
                .build();
            assert_eq!(backend.subscriptions.borrow().len(), 1);
        });
        assert!(backend.subscriptions.borrow().is_empty());
    }

    #[test]
    fn manual_mode_marks_value_dirty_until_flush() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::default();
            let value = parse_builder(scope, backend.clone(), "counter")
                .mode(PersistMode::Manual)
                .sync(SyncStrategy::None)
                .default(1)
                .build();
            value.set(2);
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::Dirty("2".to_string())
            );
            value.flush().unwrap();
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::Ready("2".to_string())
            );
        });
    }

    #[test]
    fn initial_decode_removal_failure_sets_write_error() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::failing_removes();
            backend
                .state
                .borrow_mut()
                .insert("counter".to_string(), "bad".to_string());
            let value = parse_builder(scope, backend.clone(), "counter")
                .write_default(WriteDefault::Always)
                .on_decode_error(DecodePolicy::RemoveAndUseDefault)
                .default(1)
                .build();
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::WriteError("mock backend remove failure".to_string())
            );
            assert_eq!(backend.get("counter").unwrap(), Some("bad".to_string()));
        });
    }

    #[test]
    fn external_decode_removal_failure_sets_write_error() {
        let mut runtime = silex_core::Runtime::new();
        runtime.child(|scope| {
            let backend = MockBackend::with_value("counter", "1");
            let value = parse_builder(scope, backend.clone(), "counter")
                .on_decode_error(DecodePolicy::RemoveAndUseDefault)
                .default(1)
                .build();
            *backend.fail_removes.borrow_mut() = true;
            backend.emit(
                "counter",
                BackendEvent::Set {
                    key: "counter".into(),
                    value: "bad".to_string(),
                },
            );
            assert_eq!(
                value.state().get_untracked(),
                PersistenceState::WriteError("mock backend remove failure".to_string())
            );
        });
    }
}
