use crate::{
    DecodePolicy, PersistMode, PersistenceError, PersistenceErrorKind, RemovePolicy, SyncStrategy,
    WriteDefault,
    backend::{
        BackendEventSink, LocalStorageBackend, PersistenceBackend, QueryBackend,
        SessionStorageBackend,
    },
    codec::{
        OptionCodec, ParseCodec, PersistCodec, StringCodec, map_decode_error, map_encode_error,
    },
    state::{
        OwnerDebounceState, PersistenceController, PersistenceState, Persistent,
        apply_backend_event, flush_persistent_value, invalidate_debounce,
        take_controller_resources, take_skip_next_auto_flush, take_suppress_manual_state,
    },
};
use ref_str::LocalStaticRefStr;
use silex_core::{
    CallbackInvokeError, CompletionSender, ErrorHandlerInput, ErrorHandlerToken, OwnerAccess,
    ReactiveError, RxRead, SilexError, SilexErrorKind, SilexResult,
    traits::{RxGet, RxWrite},
    unwind_safe,
};
use silex_dom::helpers::set_timeout;
use silex_dom::view::MountOwnerToken;
use silex_router::RouterContext;
use std::{
    borrow::Cow,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn submit_completion<T: 'static>(
    token: &CompletionSender<T>,
    error_token: &CompletionSender<SilexError>,
    value: T,
) {
    let result = token.submit(value);
    let Err(error) = result else {
        return;
    };
    let error = match error {
        CallbackInvokeError::Runtime(error) => SilexError::fatal(SilexErrorKind::Reactivity(error)),
        CallbackInvokeError::User(error) => error,
        CallbackInvokeError::Handler(error) => {
            SilexError::fatal(SilexErrorKind::Reactivity(ReactiveError::Handler(error)))
        }
        CallbackInvokeError::Close(error) => SilexError::fatal(SilexErrorKind::Close(error)),
    };
    let error_result = catch_unwind(AssertUnwindSafe(|| error_token.submit(error)));
    if let Ok(Err(_)) | Err(_) = error_result {
        let _ = catch_unwind(AssertUnwindSafe(|| token.cancel()));
        let _ = catch_unwind(AssertUnwindSafe(|| error_token.cancel()));
        if let Err(panic) = error_result {
            resume_unwind(panic);
        }
    }
}

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
pub struct PersistentBuilder<'scope, B, C, T = (), D = NoDefault, H = ErrorHandlerToken<'scope>>
where
    T: 'scope,
{
    owner: OwnerAccess<'scope>,
    error_handler: H,
    key: LocalStaticRefStr,
    backend: B,
    codec: C,
    config: PersistConfig<'scope, T>,
    _marker: PhantomData<D>,
}

impl<'scope, H> PersistentBuilder<'scope, NoBackend, NoCodec, (), NoDefault, H>
where
    H: ErrorHandlerInput<'scope>,
{
    pub fn new(
        owner: OwnerAccess<'scope>,
        key: impl Into<LocalStaticRefStr>,
        error_handler: H,
    ) -> Self {
        Self {
            owner,
            error_handler,
            key: key.into(),
            backend: NoBackend,
            codec: NoCodec,
            config: PersistConfig::new(),
            _marker: PhantomData,
        }
    }
}

impl<'scope, C, T, D, H> PersistentBuilder<'scope, NoBackend, C, T, D, H>
where
    T: 'scope,
    H: ErrorHandlerInput<'scope>,
{
    /// Select a custom persistence backend.
    pub fn backend<B>(self, backend: B) -> PersistentBuilder<'scope, B, C, T, D, H>
    where
        B: PersistenceBackend<'scope>,
    {
        PersistentBuilder {
            owner: self.owner,
            error_handler: self.error_handler,
            key: self.key,
            backend,
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    pub fn local(self) -> PersistentBuilder<'scope, LocalStorageBackend, C, T, D, H> {
        PersistentBuilder {
            owner: self.owner,
            error_handler: self.error_handler,
            key: self.key,
            backend: LocalStorageBackend::default(),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }

    pub fn session(self) -> PersistentBuilder<'scope, SessionStorageBackend, C, T, D, H> {
        PersistentBuilder {
            owner: self.owner,
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
        ctx: RouterContext<'scope>,
    ) -> PersistentBuilder<'scope, QueryBackend<'scope>, C, T, D, H> {
        PersistentBuilder {
            owner: self.owner,
            error_handler: self.error_handler,
            key: self.key,
            backend: QueryBackend::new(ctx),
            codec: self.codec,
            config: self.config,
            _marker: PhantomData,
        }
    }
}

impl<'scope, B, T, D, H> PersistentBuilder<'scope, B, NoCodec, T, D, H>
where
    T: 'scope,
    H: ErrorHandlerInput<'scope>,
{
    /// Select a custom codec and its persisted value type.
    pub fn custom_codec<U, C>(self, codec: C) -> PersistentBuilder<'scope, B, C, U, D, H>
    where
        C: PersistCodec<U> + 'scope,
        U: 'scope,
    {
        PersistentBuilder {
            owner: self.owner,
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

    pub fn string(self) -> PersistentBuilder<'scope, B, StringCodec, String, D, H> {
        PersistentBuilder {
            owner: self.owner,
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

    pub fn cow(self) -> PersistentBuilder<'scope, B, StringCodec, Cow<'scope, str>, D, H> {
        PersistentBuilder {
            owner: self.owner,
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

    pub fn parse<U>(self) -> PersistentBuilder<'scope, B, ParseCodec<U>, U, D, H>
    where
        U: std::fmt::Display + std::str::FromStr + Clone + 'scope,
        <U as std::str::FromStr>::Err: std::fmt::Display,
    {
        PersistentBuilder {
            owner: self.owner,
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
    pub fn json<U>(self) -> PersistentBuilder<'scope, B, crate::PersistJsonCodec<U>, U, D, H>
    where
        U: serde::Serialize + serde::de::DeserializeOwned + Clone + 'scope,
    {
        PersistentBuilder {
            owner: self.owner,
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

impl<'scope, B, C, T, D, H> PersistentBuilder<'scope, B, C, T, D, H>
where
    T: 'scope,
    H: ErrorHandlerInput<'scope>,
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

impl<'scope, B, C, T, D, H> PersistentBuilder<'scope, B, C, T, D, H>
where
    T: Clone + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    pub fn default(self, value: T) -> PersistentBuilder<'scope, B, C, T, HasDefault, H> {
        let value = Rc::new(value);
        PersistentBuilder {
            owner: self.owner,
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
    ) -> PersistentBuilder<'scope, B, C, T, HasDefault, H> {
        PersistentBuilder {
            owner: self.owner,
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

impl<'scope, B, C, T, D, H> PersistentBuilder<'scope, B, C, T, D, H>
where
    C: PersistCodec<T> + 'scope,
    T: Clone + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    pub fn optional(
        self,
    ) -> PersistentBuilder<'scope, B, OptionCodec<C, T>, Option<T>, HasDefault, H> {
        PersistentBuilder {
            owner: self.owner,
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

impl<'scope, B, C, T, H> PersistentBuilder<'scope, B, C, T, HasDefault, H>
where
    B: PersistenceBackend<'scope>,
    C: PersistCodec<T> + 'scope,
    T: Clone + PartialEq + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    pub fn build(self) -> Result<Persistent<'scope, T>, PersistenceError> {
        let key = self.key.clone();
        let error_handler = self.error_handler.handler_ref();
        let default = self.config.default.ok_or_else(|| {
            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                "persistent builder is missing a default value".to_string(),
            ))
        })?;
        let initial_default = default();
        let value = self
            .owner
            .rw_signal(initial_default.clone())
            .map_err(PersistenceError::from)?;
        let state = self
            .owner
            .rw_signal(PersistenceState::Ready(String::new()))
            .map_err(PersistenceError::from)?;
        let backend = self.backend.clone();
        let codec = self.codec.clone();
        let debounce = if matches!(self.config.mode, PersistMode::Immediate)
            && matches!(self.config.sync, SyncStrategy::Debounce(_))
        {
            Some(OwnerDebounceState::new())
        } else {
            None
        };
        let controller = self
            .owner
            .stored(PersistenceController {
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
                debounce,
                subscription: None,
            })
            .map_err(PersistenceError::from)?;

        let cleanup_controller = controller;
        self.owner
            .on_cleanup(
                move || -> SilexResult<()> {
                    let (subscription, timer) = take_controller_resources(cleanup_controller)?;
                    if let Some(timer) = &timer {
                        timer.cancel();
                    }
                    drop(timer);
                    drop(subscription);
                    Ok(())
                },
                error_handler,
            )
            .map_err(|error| {
                PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                    error.to_string(),
                ))
            })?;

        let mut had_missing_value = false;
        let mut initial_error = false;
        match backend.get(&key) {
            Ok(Some(raw)) => match self.codec.decode(&raw) {
                Ok(decoded) => {
                    value.set_untracked(decoded)?;
                    controller
                        .update_untracked(|controller| {
                            controller.last_flushed_raw = Some(raw.clone());
                        })
                        .map_err(PersistenceError::from)?;
                    state.set_untracked(PersistenceState::Ready(raw))?;
                }
                Err(message) => {
                    state.set_untracked(PersistenceState::DecodeError(crate::DecodeErrorInfo {
                        raw: raw.clone(),
                        message: message.clone(),
                    }))?;
                    value.set_untracked(default())?;
                    controller
                        .update_untracked(|controller| {
                            controller.last_flushed_raw = None;
                            controller.skip_next_auto_flush_generation = Some(0);
                            controller.suppress_manual_state_generation = Some(0);
                        })
                        .map_err(PersistenceError::from)?;
                    initial_error = true;
                    if matches!(self.config.decode_policy, DecodePolicy::RemoveAndUseDefault)
                        && let Err(error) = backend.remove(&key)
                    {
                        state.set_untracked(PersistenceState::WriteError(error.message()))?;
                    }
                }
            },
            Ok(None) => {
                had_missing_value = true;
                value.set_untracked(default())?;
                state.set_untracked(PersistenceState::Ready(String::new()))?;
            }
            Err(PersistenceError::Recoverable(PersistenceErrorKind::BackendUnavailable)) => {
                value.set_untracked(default())?;
                state.set_untracked(PersistenceState::Unavailable)?;
                controller
                    .update_untracked(|controller| {
                        controller.skip_next_auto_flush_generation = Some(0);
                        controller.suppress_manual_state_generation = Some(0);
                    })
                    .map_err(PersistenceError::from)?;
                initial_error = true;
            }
            Err(error) => {
                value.set_untracked(default())?;
                state.set_untracked(PersistenceState::ReadError(error.message()))?;
                controller
                    .update_untracked(|controller| {
                        controller.skip_next_auto_flush_generation = Some(0);
                        controller.suppress_manual_state_generation = Some(0);
                    })
                    .map_err(PersistenceError::from)?;
                initial_error = true;
            }
        }

        if had_missing_value {
            match self.config.write_default {
                WriteDefault::Never => {
                    controller
                        .update_untracked(|controller| {
                            controller.skip_next_auto_flush_generation = Some(0);
                        })
                        .map_err(PersistenceError::from)?;
                }
                WriteDefault::IfMissing | WriteDefault::Always => {
                    if flush_persistent_value(controller, value, state).is_err() {
                        initial_error = true;
                        controller
                            .update_untracked(|controller| {
                                controller.skip_next_auto_flush_generation = Some(0);
                                controller.suppress_manual_state_generation = Some(0);
                            })
                            .map_err(PersistenceError::from)?;
                    }
                }
            }
        } else if matches!(self.config.write_default, WriteDefault::Always)
            && matches!(state.get_untracked()?, PersistenceState::Ready(_))
            && flush_persistent_value(controller, value, state).is_err()
        {
            initial_error = true;
            controller
                .update_untracked(|controller| {
                    controller.skip_next_auto_flush_generation = Some(0);
                    controller.suppress_manual_state_generation = Some(0);
                })
                .map_err(PersistenceError::from)?;
        }

        if initial_error {
            controller
                .update_untracked(|controller| {
                    controller.suppress_manual_state_generation = Some(0);
                })
                .map_err(PersistenceError::from)?;
        }

        let error_lease = error_handler.lease().map_err(|error| {
            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                error.to_string(),
            ))
        })?;
        let completion_error_lease = error_lease.clone();
        let error_completion =
            self.owner
                .completion_sender(unwind_safe(move |error: SilexError| {
                    completion_error_lease
                        .handle(error)
                        .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))
                }))?;
        let mut subscription_error = None;
        if matches!(self.config.sync, SyncStrategy::CrossContext) {
            let token = self.owner.completion_sender(unwind_safe({
                move |event| {
                    apply_backend_event(controller, value, state, event);
                    Ok(())
                }
            }))?;
            let error_completion_for_sink = error_completion.clone();
            let sink: BackendEventSink = Rc::new(move |event| {
                submit_completion(&token, &error_completion_for_sink, event);
            });
            match self
                .backend
                .subscribe(self.owner, key.clone(), sink, error_handler)
            {
                Ok(binding) => {
                    controller
                        .update_untracked(|controller| {
                            controller.subscription = Some(binding);
                        })
                        .map_err(PersistenceError::from)?;
                }
                Err(error) => match error.into_error() {
                    PersistenceError::Recoverable(PersistenceErrorKind::BackendUnavailable) => {}
                    PersistenceError::Fatal(PersistenceErrorKind::InvalidConfiguration(
                        message,
                    )) => {
                        let fallback_default = initial_default.clone();
                        controller
                            .update_untracked(|controller| {
                                controller.default = Rc::new(move || fallback_default.clone());
                            })
                            .map_err(PersistenceError::from)?;
                        return Err(PersistenceError::fatal(
                            PersistenceErrorKind::InvalidConfiguration(message),
                        ));
                    }
                    error => subscription_error = Some(error.message()),
                },
            }
        }

        match self.config.mode {
            PersistMode::Immediate => {
                if let SyncStrategy::Debounce(duration) = self.config.sync {
                    let completion = self.owner.completion_sender(unwind_safe({
                        move |generation| {
                            let ready = controller.update_untracked(|controller| {
                                controller
                                    .debounce
                                    .as_mut()
                                    .is_some_and(|debounce| debounce.take_ready(generation))
                            })?;
                            if ready {
                                let _ = flush_persistent_value(controller, value, state);
                            }
                            Ok(())
                        }
                    }))?;
                    let error_completion_for_timer = error_completion.clone();
                    let _effect = self
                        .owner
                        .effect(
                            {
                                let owner_access = self.owner;
                                let owner_error_handler = error_handler;
                                move || -> SilexResult<()> {
                                    let current = value.get()?;
                                    let should_skip = take_skip_next_auto_flush(controller)?;
                                    if should_skip {
                                        return Ok(());
                                    }

                                    let encode = controller
                                        .with_untracked(|controller| controller.encode.clone())?;
                                    let raw = encode(&current);
                                    let raw = match raw {
                                        Ok(raw) => raw,
                                        Err(error) => {
                                            invalidate_debounce(controller)?;
                                            state.set(PersistenceState::WriteError(
                                                error.message(),
                                            ))?;
                                            return Ok(());
                                        }
                                    };
                                    state.set(PersistenceState::Syncing(raw))?;
                                    let (generation, timer) =
                                        controller.update(|controller| {
                                            controller
                                                .debounce
                                                .as_mut()
                                                .ok_or_else(|| {
                                                    SilexError::fatal(SilexErrorKind::Framework(
                                                        "debounce state is missing".to_string(),
                                                    ))
                                                })
                                                .map(OwnerDebounceState::begin_with_previous_timer)
                                        })??;
                                    if let Some(timer) = &timer {
                                        timer.cancel();
                                    }
                                    drop(timer);
                                    let completion = completion.clone();
                                    let error_completion = error_completion_for_timer.clone();
                                    let owner_token = MountOwnerToken::new(owner_access);
                                    match set_timeout(
                                        &owner_token,
                                        move || {
                                            submit_completion(
                                                &completion,
                                                &error_completion,
                                                generation,
                                            );
                                            Ok(())
                                        },
                                        duration,
                                        owner_error_handler,
                                    ) {
                                        Ok(timer) => {
                                            let stale_timer =
                                                controller.update(|controller| {
                                                    controller
                                                        .debounce
                                                        .as_mut()
                                                        .ok_or_else(|| {
                                                            SilexError::fatal(
                                                                SilexErrorKind::Framework(
                                                                    "debounce state is missing"
                                                                        .to_string(),
                                                                ),
                                                            )
                                                        })
                                                        .map(|debounce| {
                                                            debounce.set_timer(generation, timer)
                                                        })
                                                })??;
                                            drop(stale_timer);
                                        }
                                        Err(error) => {
                                            invalidate_debounce(controller)?;
                                            state.set(PersistenceState::WriteError(format!(
                                                "schedule persistence timeout failed: {:?}",
                                                error
                                            )))?;
                                        }
                                    }
                                    Ok(())
                                }
                            },
                            error_handler,
                        )
                        .map_err(|error| {
                            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                                error.to_string(),
                            ))
                        })?;
                } else {
                    let _effect = self
                        .owner
                        .effect(
                            move || -> SilexResult<()> {
                                value.get()?;
                                let should_skip = take_skip_next_auto_flush(controller)?;
                                if should_skip {
                                    return Ok(());
                                }
                                if let Err(error) = flush_persistent_value(controller, value, state)
                                {
                                    state.set(PersistenceState::WriteError(error.message()))?;
                                }
                                Ok(())
                            },
                            error_handler,
                        )
                        .map_err(|error| {
                            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                                error.to_string(),
                            ))
                        })?;
                }
            }
            PersistMode::Manual => {
                let _effect = self
                    .owner
                    .effect(
                        move || -> SilexResult<()> {
                            let current = value.get()?;
                            let suppress = take_suppress_manual_state(controller)?;
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
                                })?;
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
                                            state.set(PersistenceState::Ready(String::new()))?;
                                        } else {
                                            state.set(PersistenceState::Ready(raw))?;
                                        }
                                    } else {
                                        state.set(PersistenceState::Dirty(raw))?;
                                    }
                                }
                                Err(error) => {
                                    state.set(PersistenceState::WriteError(error.message()))?
                                }
                            }
                            Ok(())
                        },
                        error_handler,
                    )
                    .map_err(|error| {
                        PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                            error.to_string(),
                        ))
                    })?;
            }
        }

        if let Some(message) = subscription_error {
            state.set_untracked(PersistenceState::WriteError(message))?;
        }

        Ok(Persistent {
            owner: self.owner,
            value,
            state,
            controller,
        })
    }
}
