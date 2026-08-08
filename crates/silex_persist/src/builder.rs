use crate::{
    DecodePolicy, PersistMode, PersistenceError, RemovePolicy, SyncStrategy, WriteDefault,
    backend::{
        BackendEvent, BackendEventSink, LocalStorageBackend, PersistenceBackend, QueryBackend,
        SessionStorageBackend,
    },
    codec::{
        OptionCodec, ParseCodec, PersistCodec, StringCodec, map_decode_error, map_encode_error,
    },
    state::{
        PersistenceController, PersistenceState, Persistent, ScopedDebounceState,
        apply_backend_event, flush_persistent_value, take_skip_next_auto_flush,
        take_suppress_manual_state,
    },
};
use ref_str::LocalStaticRefStr;
use silex_core::{
    ErrorReporter, RxRead, Scope, SilexResult,
    traits::{RxGet, RxWrite},
    unwind_safe,
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
    error_handler: ErrorReporter<'scope>,
    key: LocalStaticRefStr,
    backend: B,
    codec: C,
    config: PersistConfig<'scope, T>,
    _marker: PhantomData<D>,
}

impl<'scope> PersistentBuilder<'scope, NoBackend, NoCodec, (), NoDefault> {
    pub fn new(
        scope: Scope<'scope>,
        key: impl Into<LocalStaticRefStr>,
        error_handler: ErrorReporter<'scope>,
    ) -> Self {
        Self {
            scope,
            error_handler,
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
    /// Select a custom persistence backend.
    pub fn backend<B>(self, backend: B) -> PersistentBuilder<'scope, B, C, T, D>
    where
        B: PersistenceBackend<'scope>,
    {
        PersistentBuilder {
            scope: self.scope,
            error_handler: self.error_handler,
            key: self.key,
            backend,
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    pub fn local(self) -> PersistentBuilder<'scope, LocalStorageBackend, C, T, D> {
        PersistentBuilder {
            scope: self.scope,
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
    /// Select a custom codec and its persisted value type.
    pub fn custom_codec<U, C>(self, codec: C) -> PersistentBuilder<'scope, B, C, U, D>
    where
        C: PersistCodec<U> + 'scope,
        U: 'scope,
    {
        PersistentBuilder {
            scope: self.scope,
            error_handler: self.error_handler,
            key: self.key,
            backend: self.backend,
            codec,
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

    pub fn string(self) -> PersistentBuilder<'scope, B, StringCodec, String, D> {
        PersistentBuilder {
            scope: self.scope,
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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
            error_handler: self.error_handler,
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

        let key = self.key.clone();
        let subscription = Rc::new(RefCell::new(None));
        let pending_events = Rc::new(RefCell::new(Vec::<BackendEvent>::new()));
        let pending_sink = Rc::new(RefCell::new(None::<BackendEventSink>));
        let mut subscription_error = None;
        if matches!(self.config.sync, SyncStrategy::CrossContext) {
            let pending_events_for_sink = pending_events.clone();
            let pending_sink_for_sink = pending_sink.clone();
            let sink: BackendEventSink = Rc::new(move |event| {
                let sink = pending_sink_for_sink.borrow().clone();
                if let Some(sink) = sink {
                    sink(event);
                } else {
                    pending_events_for_sink.borrow_mut().push(event);
                }
            });
            match self
                .backend
                .subscribe(self.scope, key.clone(), sink, self.error_handler)
            {
                Ok(binding) => *subscription.borrow_mut() = Some(binding),
                Err(error) => match error.into_error() {
                    PersistenceError::BackendUnavailable => {}
                    PersistenceError::InvalidConfiguration(message) => {
                        return Err(PersistenceError::InvalidConfiguration(message));
                    }
                    error => subscription_error = Some(error.message()),
                },
            }
        }

        let subscription_for_cleanup = subscription.clone();
        self.scope
            .on_cleanup(
                move || -> SilexResult<()> {
                    subscription_for_cleanup.borrow_mut().take();
                    Ok(())
                },
                self.error_handler,
            )
            .map_err(|error| PersistenceError::InvalidConfiguration(error.to_string()))?;

        let default = self
            .config
            .default
            .expect("Default status verified by typestate");
        let value = self.scope.rw_signal(default());
        let state = self.scope.rw_signal(PersistenceState::Ready(String::new()));
        let backend = self.backend.clone();
        let codec = self.codec.clone();
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
            value_generation: 0,
            skip_next_auto_flush_generation: None,
            suppress_manual_state_generation: None,
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
            debounce: debounce.clone(),
        });

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
                        controller.skip_next_auto_flush_generation = Some(0);
                        controller.suppress_manual_state_generation = Some(0);
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
                    controller.skip_next_auto_flush_generation = Some(0);
                    controller.suppress_manual_state_generation = Some(0);
                });
                initial_error = true;
            }
            Err(error) => {
                value.set_untracked(default());
                state.set_untracked(PersistenceState::ReadError(error.message()));
                let _ = controller.try_update_untracked(|controller| {
                    controller.skip_next_auto_flush_generation = Some(0);
                    controller.suppress_manual_state_generation = Some(0);
                });
                initial_error = true;
            }
        }

        if had_missing_value {
            match self.config.write_default {
                WriteDefault::Never => {
                    let _ = controller.try_update_untracked(|controller| {
                        controller.skip_next_auto_flush_generation = Some(0);
                    });
                }
                WriteDefault::IfMissing | WriteDefault::Always => {
                    if flush_persistent_value(controller, value, state).is_err() {
                        initial_error = true;
                        let _ = controller.try_update_untracked(|controller| {
                            controller.skip_next_auto_flush_generation = Some(0);
                            controller.suppress_manual_state_generation = Some(0);
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
                controller.skip_next_auto_flush_generation = Some(0);
                controller.suppress_manual_state_generation = Some(0);
            });
        }

        if initial_error {
            let _ = controller.try_update_untracked(|controller| {
                controller.suppress_manual_state_generation = Some(0);
            });
        }

        if let Some(message) = subscription_error {
            state.set_untracked(PersistenceState::WriteError(message));
        }

        if matches!(self.config.sync, SyncStrategy::CrossContext) {
            let token = self.scope.completion_sender(unwind_safe({
                move |event| {
                    apply_backend_event(controller, value, state, event);
                }
            }));
            let sink: BackendEventSink = Rc::new(move |event| {
                let _ = token.submit(event);
            });
            *pending_sink.borrow_mut() = Some(sink.clone());
            let pending_events = std::mem::take(&mut *pending_events.borrow_mut());
            for event in pending_events {
                sink(event);
            }
        }

        match self.config.mode {
            PersistMode::Immediate => {
                if let SyncStrategy::Debounce(duration) = self.config.sync {
                    let debounce_state = debounce
                        .clone()
                        .expect("debounce state must exist for debounce mode");
                    let debounce_for_completion = debounce_state.clone();
                    let completion = self.scope.completion_sender(unwind_safe({
                        move |generation| {
                            if debounce_for_completion.borrow_mut().take_ready(generation) {
                                let _ = flush_persistent_value(controller, value, state);
                            }
                        }
                    }));
                    let debounce_for_cleanup = debounce_state.clone();
                    self.scope
                        .on_cleanup(
                            move || -> SilexResult<()> {
                                debounce_for_cleanup.borrow_mut().invalidate();
                                Ok(())
                            },
                            self.error_handler,
                        )
                        .map_err(|error| {
                            PersistenceError::InvalidConfiguration(error.to_string())
                        })?;
                    let _effect = self
                        .scope
                        .effect(
                            move || -> SilexResult<()> {
                                let current = value.try_get()?;
                                let should_skip = take_skip_next_auto_flush(controller);
                                if should_skip {
                                    return Ok(());
                                }

                                let encode = controller
                                    .with_untracked(|controller| controller.encode.clone());
                                let raw = encode(&current);
                                let raw = match raw {
                                    Ok(raw) => raw,
                                    Err(error) => {
                                        debounce_state.borrow_mut().invalidate();
                                        state.set(PersistenceState::WriteError(error.message()));
                                        return Ok(());
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
                                        let _ = debounce_state
                                            .borrow_mut()
                                            .set_timer(generation, timer);
                                    }
                                    Err(error) => {
                                        debounce_state.borrow_mut().invalidate();
                                        state.set(PersistenceState::WriteError(format!(
                                            "schedule persistence timeout failed: {:?}",
                                            error
                                        )));
                                    }
                                }
                                Ok(())
                            },
                            self.error_handler,
                        )
                        .map_err(|error| {
                            PersistenceError::InvalidConfiguration(error.to_string())
                        })?;
                } else {
                    let _effect = self
                        .scope
                        .effect(
                            move || -> SilexResult<()> {
                                value.try_get()?;
                                let should_skip = take_skip_next_auto_flush(controller);
                                if should_skip {
                                    return Ok(());
                                }
                                if let Err(error) = flush_persistent_value(controller, value, state)
                                {
                                    state.set(PersistenceState::WriteError(error.message()));
                                }
                                Ok(())
                            },
                            self.error_handler,
                        )
                        .map_err(|error| {
                            PersistenceError::InvalidConfiguration(error.to_string())
                        })?;
                }
            }
            PersistMode::Manual => {
                let _effect = self
                    .scope
                    .effect(
                        move || -> SilexResult<()> {
                            let current = value.try_get()?;
                            let suppress = take_suppress_manual_state(controller);
                            if suppress {
                                return Ok(());
                            }

                            let (encode, default, last_raw) =
                                controller.with_untracked(|controller| {
                                    (
                                        controller.encode.clone(),
                                        controller.default.clone(),
                                        controller.last_flushed_raw.clone(),
                                    )
                                });
                            let raw = encode(&current);
                            let is_default = current == default();
                            match raw {
                                Ok(raw) => {
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
                                Err(error) => {
                                    state.set(PersistenceState::WriteError(error.message()))
                                }
                            }
                            Ok(())
                        },
                        self.error_handler,
                    )
                    .map_err(|error| PersistenceError::InvalidConfiguration(error.to_string()))?;
            }
        }

        Ok(Persistent {
            scope: self.scope,
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
