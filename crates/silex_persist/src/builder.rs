use crate::{
    DecodePolicy, PersistExternalSync, PersistWriteMode, PersistenceError, PersistenceErrorKind,
    RemovePolicy, WriteDefault,
    backend::{
        BackendEventSink, LocalStorageBackend, PersistenceBackend, QueryBackend,
        SessionStorageBackend,
    },
    codec::{
        OptionCodec, ParseCodec, PersistCodec, StringCodec, map_decode_error, map_encode_error,
    },
    runtime::{PersistRuntime, WriteOrigin},
    state::{
        PersistenceController, PersistenceState, Persistent, apply_backend_event,
        commit_persisted_request, invalidate_debounce, persist_current_value,
        take_controller_resources, take_local_mutation,
    },
};
use ref_str::LocalStaticRefStr;
use silex_core::{
    CallbackInvokeError, CompletionSender, EffectPhase, ErrorHandlerInput, ErrorHandlerToken,
    OwnerAccess, ReactiveError, RxReadRef, SilexError, SilexErrorKind, SilexResult,
    traits::{RxGet, RxWrite},
    unwind_safe,
};
use silex_dom::view::{MountOwnerToken, OwnedTimeout};
use silex_router::RouterContext;
use std::{
    borrow::Cow,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
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
    let (callback, close) = error.into_parts();
    let mut failures = false;
    let mut submit_error =
        |error| match catch_unwind(AssertUnwindSafe(|| error_token.submit(error))) {
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => failures = true,
        };
    if let Some(callback) = callback {
        submit_error(match callback {
            CallbackInvokeError::Runtime(error) => {
                SilexError::fatal(SilexErrorKind::Reactivity(error))
            }
            CallbackInvokeError::User(error) => error,
            CallbackInvokeError::Handler(error) => {
                SilexError::fatal(SilexErrorKind::Reactivity(ReactiveError::Handler(error)))
            }
        });
    }
    if let Some(close) = close {
        submit_error(SilexError::fatal(SilexErrorKind::Close(close)));
    }
    if failures {
        let _ = catch_unwind(AssertUnwindSafe(|| token.cancel()));
        let _ = catch_unwind(AssertUnwindSafe(|| error_token.cancel()));
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
    write_mode: PersistWriteMode,
    external_sync: PersistExternalSync,
}

impl<'scope, T: 'scope> PersistConfig<'scope, T> {
    fn new() -> Self {
        Self {
            default: None,
            write_default: WriteDefault::IfMissing,
            decode_policy: DecodePolicy::RemoveAndUseDefault,
            remove_policy: RemovePolicy::UseDefault,
            write_mode: PersistWriteMode::Immediate,
            external_sync: PersistExternalSync::StorageEvents,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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

    pub fn write_mode(mut self, mode: PersistWriteMode) -> Self {
        self.config.write_mode = mode;
        self
    }

    pub fn external_sync(mut self, sync: PersistExternalSync) -> Self {
        self.config.external_sync = sync;
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
                write_mode: self.config.write_mode,
                external_sync: self.config.external_sync,
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
    H: ErrorHandlerInput<'scope> + 'scope,
{
    pub fn build(self) -> Result<Persistent<'scope, T>, PersistenceError> {
        let key = self.key.clone();
        let error_handler_input: Rc<dyn ErrorHandlerInput<'scope> + 'scope> =
            Rc::new(self.error_handler);
        let error_handler = error_handler_input.handler_ref();
        let error_lease = error_handler.lease().map_err(|error| {
            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                error.to_string(),
            ))
        })?;
        let default = self.config.default.ok_or_else(|| {
            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                "persistent builder is missing a default value".to_string(),
            ))
        })?;
        let initial_default = default();
        let value = self
            .owner
            .signal(initial_default.clone())
            .map_err(PersistenceError::from)?;
        let state = self
            .owner
            .signal(PersistenceState::Ready(String::new()))
            .map_err(PersistenceError::from)?;
        let backend = self.backend.clone();
        let codec = self.codec.clone();
        let controller = self
            .owner
            .stored(PersistenceController {
                key: key.clone(),
                default: default.clone(),
                decode_policy: self.config.decode_policy,
                remove_policy: self.config.remove_policy,
                runtime: PersistRuntime::new(),
                local_mutation_pending: false,
                error_handler: error_handler_input.clone(),
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
                subscription: None,
            })
            .map_err(PersistenceError::from)?;

        let cleanup_controller = controller;
        self.owner
            .on_cleanup(
                move || -> SilexResult<()> {
                    let (subscription, timer) = take_controller_resources(cleanup_controller)?;
                    if let Some(timer) = &timer {
                        timer.cancel()?;
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
        match backend.get(&key) {
            Ok(Some(raw)) => match self.codec.decode(&raw) {
                Ok(decoded) => {
                    value.set_untracked(decoded)?;
                    controller
                        .update_untracked(|controller| {
                            controller.runtime.initialize_snapshot(Some(raw.clone()));
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
                            controller.runtime.initialize_snapshot(None);
                        })
                        .map_err(PersistenceError::from)?;
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
                controller.update_untracked(|controller| {
                    controller.runtime.initialize_snapshot(None);
                })?;
                state.set_untracked(PersistenceState::Ready(String::new()))?;
            }
            Err(PersistenceError::Recoverable(PersistenceErrorKind::BackendUnavailable)) => {
                value.set_untracked(default())?;
                controller.update_untracked(|controller| {
                    controller.runtime.initialize_snapshot(None);
                })?;
                state.set_untracked(PersistenceState::Unavailable)?;
            }
            Err(error) => {
                value.set_untracked(default())?;
                controller.update_untracked(|controller| {
                    controller.runtime.initialize_snapshot(None);
                })?;
                state.set_untracked(PersistenceState::ReadError(error.message()))?;
            }
        }

        if had_missing_value {
            match self.config.write_default {
                WriteDefault::Never => {}
                WriteDefault::IfMissing | WriteDefault::Always => {
                    let _ = persist_current_value(controller, value, state, WriteOrigin::Bootstrap);
                }
            }
        } else if matches!(self.config.write_default, WriteDefault::Always)
            && matches!(state.get_untracked()?, PersistenceState::Ready(_))
            && persist_current_value(controller, value, state, WriteOrigin::Bootstrap).is_err()
        {
        }

        let completion_error_lease = error_lease.clone();
        let error_completion =
            self.owner
                .completion_sender(unwind_safe(move |error: SilexError| {
                    completion_error_lease
                        .handle(error)
                        .map_err(|error| SilexError::fatal(ReactiveError::Handler(error)))
                }))?;
        let mut subscription_error = None;
        if !matches!(self.config.external_sync, PersistExternalSync::Disabled) {
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
                Ok(mut binding) => {
                    let close_error_completion = error_completion.clone();
                    binding.set_error_sink(Rc::new(move |error| {
                        submit_completion(
                            &close_error_completion,
                            &close_error_completion,
                            error.into(),
                        );
                    }));
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

        match self.config.write_mode {
            PersistWriteMode::Debounced(duration) => {
                let _effect = self
                    .owner
                    .effect(
                        EffectPhase::Normal,
                        {
                            let owner_access = self.owner;
                            move || -> SilexResult<()> {
                                let current = value.get()?;
                                if !take_local_mutation(controller)? {
                                    return Ok(());
                                }

                                let encode = controller
                                    .with_untracked(|controller| controller.encode.clone())?;
                                let raw = encode(&current);
                                let raw = match raw {
                                    Ok(raw) => raw,
                                    Err(error) => {
                                        invalidate_debounce(controller)?;
                                        state.set(PersistenceState::WriteError(error.message()))?;
                                        return Ok(());
                                    }
                                };
                                state.set(PersistenceState::Syncing(raw.clone()))?;
                                let (ticket, previous_timer) =
                                    controller.update_untracked(|controller| {
                                        controller
                                            .runtime
                                            .begin_request(
                                                Some(raw.clone()),
                                                WriteOrigin::LocalMutation,
                                            )
                                            .ok_or_else(|| {
                                                SilexError::fatal(SilexErrorKind::Framework(
                                                    "persistence runtime is closed".to_string(),
                                                ))
                                            })
                                    })??;
                                if let Some(timer) = previous_timer
                                    && let Err(error) = timer.cancel()
                                {
                                    let (current, timer) =
                                        controller.update_untracked(|controller| {
                                            controller
                                                .runtime
                                                .mark_schedule_failed(ticket, error.to_string())
                                        })?;
                                    if let Some(timer) = timer {
                                        timer.cancel()?;
                                    }
                                    if current {
                                        state
                                            .set(PersistenceState::WriteError(error.to_string()))?;
                                    }
                                    return Ok(());
                                }
                                let owner_token = MountOwnerToken::new(owner_access);
                                let owner_error_handler =
                                    controller.with_untracked(|controller| {
                                        controller.error_handler.handler_ref()
                                    })?;
                                match OwnedTimeout::schedule(
                                    &owner_token,
                                    move || {
                                        let request =
                                            controller.update_untracked(|controller| {
                                                controller.runtime.claim_timer(ticket)
                                            })?;
                                        if let Some(request) = request {
                                            commit_persisted_request(controller, state, request)
                                                .map_err(SilexError::from)?;
                                        }
                                        Ok(())
                                    },
                                    duration,
                                    owner_error_handler,
                                ) {
                                    Ok(timer) => {
                                        let stale_timer =
                                            controller.update_untracked(|controller| {
                                                controller.runtime.attach_timer(ticket, timer)
                                            })?;
                                        if let Some(timer) = stale_timer {
                                            timer.cancel()?;
                                        }
                                    }
                                    Err(error) => {
                                        let message = format!(
                                            "schedule persistence timeout failed: {:?}",
                                            error
                                        );
                                        let (current, timer) =
                                            controller.update_untracked(|controller| {
                                                controller
                                                    .runtime
                                                    .mark_schedule_failed(ticket, message.clone())
                                            })?;
                                        if let Some(timer) = timer {
                                            timer.cancel()?;
                                        }
                                        if current {
                                            state.set(PersistenceState::WriteError(message))?;
                                        }
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
            }
            PersistWriteMode::Immediate => {
                let _effect = self
                    .owner
                    .effect(
                        EffectPhase::Normal,
                        move || -> SilexResult<()> {
                            value.get()?;
                            if !take_local_mutation(controller)? {
                                return Ok(());
                            }
                            if let Err(error) = persist_current_value(
                                controller,
                                value,
                                state,
                                WriteOrigin::LocalMutation,
                            ) {
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
            PersistWriteMode::Manual => {
                let _effect = self
                    .owner
                    .effect(
                        EffectPhase::Normal,
                        move || -> SilexResult<()> {
                            let current = value.get()?;
                            if !take_local_mutation(controller)? {
                                return Ok(());
                            }

                            let (encode, default, last_raw) =
                                controller.with_untracked(|controller| {
                                    (
                                        controller.encode.clone(),
                                        controller.default.clone(),
                                        controller.runtime.last_backend_raw(),
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
