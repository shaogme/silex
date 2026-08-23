use crate::backend::{BackendEvent, BackendSubscription};
use crate::builder::PersistentBuilder;
use crate::runtime::{PersistRuntime, WriteOrigin, WriteRequest, WriteToken};
use crate::{
    DecodePolicy, NoBackend, NoCodec, NoDefault, PersistenceError, PersistenceErrorKind,
    RemovePolicy,
};
use ref_str::LocalStaticRefStr;
use silex_core::{
    ErrorHandlerInput, OwnerAccess, ReactiveError, Rx, RxGet, SilexError, SilexErrorKind,
    SilexResult, StoreField,
    reactivity::{PromotionPlan, ReactiveSource, ReadSignal, Signal, StoredValue},
    traits::{RxBase, RxCloneData, RxData, RxRead, RxValue, RxWrite},
};
use silex_dom::view::{MountContext, MountInstance, OwnedTimeout, View};
use std::rc::Rc;

pub type PersistenceGetFn<'scope> =
    Rc<dyn Fn(&str) -> Result<Option<String>, PersistenceError> + 'scope>;
pub type PersistenceSetFn<'scope> = Rc<dyn Fn(&str, &str) -> Result<(), PersistenceError> + 'scope>;
pub type PersistenceRemoveFn<'scope> = Rc<dyn Fn(&str) -> Result<(), PersistenceError> + 'scope>;
pub type PersistenceEncodeFn<'scope, T> =
    Rc<dyn Fn(&T) -> Result<String, PersistenceError> + 'scope>;
pub type PersistenceDecodeFn<'scope, T> = Rc<dyn Fn(&str) -> Result<T, PersistenceError> + 'scope>;

#[derive(Clone, Debug, PartialEq)]
pub struct DecodeErrorInfo {
    pub raw: String,
    pub message: String,
}

/// Observable state of a persistent binding.
#[derive(Clone, Debug, PartialEq)]
pub enum PersistenceState {
    /// The value is synchronized with the backend. The string is the encoded
    /// backend representation, or empty when the backend has no value.
    Ready(String),
    /// A local value has changed and has not completed persistence yet.
    Dirty(String),
    /// A local write is waiting for a debounce timer or is being committed.
    Syncing(String),
    /// The backend is unavailable in the current environment.
    Unavailable,
    /// Reading the initial backend value failed.
    ReadError(String),
    /// The backend value could not be decoded under the configured policy.
    DecodeError(DecodeErrorInfo),
    /// A write failed. The latest request remains available to retry with
    /// [`Persistent::flush`].
    WriteError(String),
}

pub(crate) struct PersistenceController<'scope, T: 'scope> {
    pub key: LocalStaticRefStr,
    pub default: Rc<dyn Fn() -> T + 'scope>,
    pub decode_policy: DecodePolicy,
    pub remove_policy: RemovePolicy,
    pub runtime: PersistRuntime<'scope>,
    pub local_mutation_pending: bool,
    pub error_handler: Rc<dyn ErrorHandlerInput<'scope> + 'scope>,
    pub backend_get: PersistenceGetFn<'scope>,
    pub backend_set: PersistenceSetFn<'scope>,
    pub backend_remove: PersistenceRemoveFn<'scope>,
    pub encode: PersistenceEncodeFn<'scope, T>,
    pub decode: PersistenceDecodeFn<'scope, T>,
    pub should_remove: Rc<dyn Fn(&T) -> bool + 'scope>,
    pub subscription: Option<BackendSubscription<'scope>>,
}

pub struct Persistent<'scope, T> {
    pub(crate) owner: OwnerAccess<'scope>,
    pub(crate) value: Signal<'scope, T>,
    pub(crate) state: Signal<'scope, PersistenceState>,
    pub(crate) controller: StoredValue<'scope, PersistenceController<'scope, T>>,
}

impl<'scope, T: 'scope> StoreField<'scope, T> for Persistent<'scope, T> {}

impl<'scope> Persistent<'scope, ()> {
    /// Starts a new persistent binding builder for the given backend key.
    pub fn builder<H>(
        owner: OwnerAccess<'scope>,
        key: impl Into<LocalStaticRefStr>,
        error_handler: H,
    ) -> PersistentBuilder<'scope, NoBackend, NoCodec, (), NoDefault, H>
    where
        H: ErrorHandlerInput<'scope>,
    {
        PersistentBuilder::new(owner, key, error_handler)
    }
}

impl<'scope, T> Clone for Persistent<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> Copy for Persistent<'scope, T> {}

impl<'scope, T> Persistent<'scope, T> {
    pub fn signal(&self) -> Signal<'scope, T> {
        self.value
    }
}

impl<'scope, T> Persistent<'scope, T>
where
    T: Clone + PartialEq + 'scope,
{
    pub fn get(&self) -> Result<T, PersistenceError> {
        self.value.get().map_err(PersistenceError::from)
    }

    pub fn get_untracked(&self) -> Result<T, PersistenceError> {
        self.value.get_untracked().map_err(PersistenceError::from)
    }

    pub fn set(&self, value: T) -> Result<(), PersistenceError> {
        self.update(|current| *current = value).map(|_| ())
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> Result<U, PersistenceError> {
        mark_local_value_write(self.controller)?;
        self.value
            .write_signal()
            .update(f)
            .map_err(PersistenceError::from)
    }

    fn validate_owner(&self) -> Result<(), PersistenceError> {
        if self.owner.is_active().map_err(PersistenceError::from)? {
            Ok(())
        } else {
            Err(PersistenceError::Fatal(PersistenceErrorKind::Reactivity(
                ReactiveError::NoSuchNode,
            )))
        }
    }

    pub fn state(&self) -> ReadSignal<'scope, PersistenceState> {
        self.state.read_signal()
    }

    pub fn key(&self) -> Result<String, PersistenceError> {
        self.controller
            .with_untracked(|controller| controller.key.to_string())
            .map_err(PersistenceError::from)
    }

    /// Return whether the backend supplied or accepted a persisted value.
    ///
    /// This is distinct from [`get_untracked`](Self::get_untracked): the latter
    /// intentionally returns the configured default when storage is empty.
    pub fn has_persisted_value(&self) -> Result<bool, PersistenceError> {
        self.controller
            .with_untracked(|controller| controller.runtime.last_backend_raw().is_some())
            .map_err(PersistenceError::from)
    }

    pub fn reset(&self) -> Result<(), PersistenceError> {
        let default = match self
            .controller
            .with(|controller| controller.default.clone())
        {
            Ok(default) => default,
            Err(error)
                if matches!(
                    error.kind(),
                    SilexErrorKind::Reactivity(ReactiveError::NoSuchNode)
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(PersistenceError::from(error)),
        };
        match self.set(default()) {
            Err(PersistenceError::Fatal(PersistenceErrorKind::Reactivity(
                ReactiveError::NoSuchNode,
            ))) => Ok(()),
            result => result,
        }
    }

    /// Remove the backend value and reset the binding to its configured
    /// default.
    pub fn remove(&self) -> Result<(), PersistenceError> {
        self.validate_owner()?;
        invalidate_debounce(self.controller).map_err(PersistenceError::from)?;
        let key = self.key()?;
        let remove_backend = self
            .controller
            .with_untracked(|controller| controller.backend_remove.clone())
            .map_err(PersistenceError::from)?;
        let result = remove_backend(&key);
        match result {
            Ok(()) => {
                let timer = self.controller.update_untracked(|controller| {
                    controller.local_mutation_pending = false;
                    controller.runtime.apply_external_snapshot(None)
                })?;
                cancel_timer(timer)?;
                self.state
                    .set(PersistenceState::Ready(String::new()))
                    .map_err(PersistenceError::from)?;
                Ok(())
            }
            Err(err) => {
                self.state
                    .set(PersistenceState::WriteError(err.message()))
                    .map_err(PersistenceError::from)?;
                Err(err)
            }
        }
    }

    /// Reload the backend snapshot and invalidate any pending local request.
    pub fn reload(&self) -> Result<(), PersistenceError> {
        self.validate_owner()?;
        let result = reload_persistent(self.controller, self.value, self.state);
        if let Err(error) = &result {
            set_error_state(self.state, error);
        }
        result
    }

    /// Persist the current local value immediately.
    ///
    /// This cancels a pending debounce timer and is also the explicit retry
    /// path after [`PersistenceState::WriteError`]. If an external snapshot
    /// arrives first, it invalidates the older local request and becomes the
    /// new baseline.
    pub fn flush(&self) -> Result<(), PersistenceError> {
        self.validate_owner()?;
        flush_persistent_value(self.controller, self.value, self.state)
    }
}

impl<'scope, T: RxData> RxValue for Persistent<'scope, T> {
    type Value = T;
}

impl<'scope, T: RxData> RxBase for Persistent<'scope, T> {
    fn track(&self) -> SilexResult<()> {
        self.value.track()
    }
}

impl<'scope, T: RxData> RxRead for Persistent<'scope, T> {
    type ReadGuard<'a>
        = <Signal<'scope, T> as RxRead>::ReadGuard<'a>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.value.read()
    }

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
        self.value.read_untracked()
    }

    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
        self.value.with(f)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
        self.value.with_untracked(f)
    }
}

impl<'scope, T: RxData> RxWrite for Persistent<'scope, T> {
    type WriteGuard<'a>
        = <Signal<'scope, T> as RxWrite>::WriteGuard<'a>
    where
        Self: 'a;

    fn write(&self) -> SilexResult<Self::WriteGuard<'_>> {
        mark_local_value_write(self.controller)?;
        self.value.write()
    }

    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> SilexResult<U> {
        mark_local_value_write(self.controller)?;
        self.value.write_signal().update(f)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        self.value.write_signal().notify()
    }
}

impl<'scope, T> ReactiveSource<'scope> for Persistent<'scope, T>
where
    T: Sized + RxData + 'scope,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + RxData + 'scope,
    {
        self.value.into_promotion_plan()
    }
}

impl<'scope, T: 'scope> From<Persistent<'scope, T>> for Signal<'scope, T> {
    fn from(value: Persistent<'scope, T>) -> Self {
        value.signal()
    }
}

impl<'scope, T> View<'scope> for Persistent<'scope, T>
where
    T: RxCloneData + 'scope,
    Rx<'scope, T>: View<'scope>,
{
    fn mount(
        &self,
        context: &MountContext<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        self.value.into_rx().mount(context)
    }
}

pub(crate) fn flush_persistent_value<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    persist_current_value(controller, value, state, WriteOrigin::ExplicitFlush)
}

pub(crate) fn persist_current_value<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
    origin: WriteOrigin,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    invalidate_debounce(controller).map_err(PersistenceError::from)?;
    let current = value
        .read_signal()
        .get_untracked()
        .map_err(PersistenceError::from)?;
    let (should_remove, encode) = controller
        .with_untracked(|controller| (controller.should_remove.clone(), controller.encode.clone()))
        .map_err(PersistenceError::from)?;
    let raw = if should_remove(&current) {
        None
    } else {
        match encode(&current) {
            Ok(raw) => Some(raw),
            Err(error) => {
                state
                    .set(PersistenceState::WriteError(error.message()))
                    .map_err(PersistenceError::from)?;
                return Err(error);
            }
        }
    };
    let (token, previous_timer) = controller.update_untracked(|controller| {
        controller
            .runtime
            .begin_request(raw.clone(), origin)
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Framework(
                    "persistence runtime is closed".to_string(),
                ))
            })
    })??;
    cancel_timer(previous_timer)?;
    let request = controller
        .update_untracked(|controller| controller.runtime.claim_request(token))?
        .ok_or_else(|| {
            PersistenceError::fatal(PersistenceErrorKind::InvalidConfiguration(
                "persistence request was superseded before commit".to_string(),
            ))
        })?;
    commit_persisted_request(controller, state, request)
}

fn commit_request<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    state: Signal<'scope, PersistenceState>,
    token: WriteToken,
    raw: Option<String>,
) -> Result<(), PersistenceError>
where
    T: 'scope,
{
    let (key, last_raw, set_backend, remove_backend) = controller
        .with_untracked(|controller| {
            (
                controller.key.clone(),
                controller.runtime.last_backend_raw(),
                controller.backend_set.clone(),
                controller.backend_remove.clone(),
            )
        })
        .map_err(PersistenceError::from)?;
    let needs_write = raw != last_raw;
    let result = if !needs_write {
        Ok(())
    } else {
        match raw.as_deref() {
            Some(raw) => set_backend(&key, raw),
            None => remove_backend(&key),
        }
    };
    if let Err(error) = result {
        let current = controller
            .update_untracked(|controller| {
                controller.runtime.mark_write_failed(token, error.message())
            })
            .map_err(PersistenceError::from)?;
        if current {
            state
                .set(PersistenceState::WriteError(error.message()))
                .map_err(PersistenceError::from)?;
        }
        return Err(error);
    }
    let current = controller
        .update_untracked(|controller| controller.runtime.mark_write_succeeded(token))
        .map_err(PersistenceError::from)?;
    if current {
        state
            .set(PersistenceState::Ready(raw.unwrap_or_default()))
            .map_err(PersistenceError::from)?;
    }
    Ok(())
}

pub(crate) fn commit_persisted_request<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    state: Signal<'scope, PersistenceState>,
    request: WriteRequest,
) -> Result<(), PersistenceError>
where
    T: 'scope,
{
    commit_request(controller, state, request.token, request.raw)
}

fn cancel_timer<'scope>(timer: Option<OwnedTimeout<'scope>>) -> Result<(), PersistenceError> {
    if let Some(timer) = timer {
        timer.cancel().map_err(PersistenceError::from)?;
    }
    Ok(())
}

pub(crate) fn reload_persistent<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    let (key, backend_get) = controller
        .with_untracked(|controller| (controller.key.clone(), controller.backend_get.clone()))
        .map_err(PersistenceError::from)?;
    let raw = backend_get(&key)?;
    apply_backend_snapshot(controller, value, state, raw)
}

pub(crate) fn apply_backend_event<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
    event: BackendEvent,
) where
    T: Clone + PartialEq + 'scope,
{
    let event_key_matches = match &event {
        BackendEvent::Set { key, .. } | BackendEvent::Removed { key } => {
            match controller.with_untracked(|controller| controller.key == *key) {
                Ok(matches) => matches,
                Err(error) => {
                    set_error_state(state, &PersistenceError::from(error));
                    return;
                }
            }
        }
        BackendEvent::ExternalRefresh => true,
    };
    if !event_key_matches {
        return;
    }

    let result = match event {
        BackendEvent::Set { value: raw, .. } => apply_raw_value(controller, value, state, raw),
        BackendEvent::Removed { .. } => apply_remove_policy(controller, value, state),
        BackendEvent::ExternalRefresh => reload_persistent(controller, value, state),
    };

    if let Err(error) = result {
        set_error_state(state, &error);
    }
}

fn apply_backend_snapshot<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
    raw: Option<String>,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    match raw {
        Some(raw) => apply_raw_value(controller, value, state, raw),
        None => apply_remove_policy(controller, value, state),
    }
}

fn apply_raw_value<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
    raw: String,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    invalidate_debounce(controller).map_err(PersistenceError::from)?;
    let decode = controller
        .with_untracked(|controller| controller.decode.clone())
        .map_err(PersistenceError::from)?;
    let decode_result = decode(&raw);
    match decode_result {
        Ok(decoded) => {
            let value_changed = value.get_untracked().map_err(PersistenceError::from)? != decoded;
            apply_external_runtime_snapshot(controller, Some(raw.clone()))
                .map_err(PersistenceError::from)?;
            if value_changed {
                value.set(decoded).map_err(PersistenceError::from)?;
            }
            state
                .set(PersistenceState::Ready(raw))
                .map_err(PersistenceError::from)?;
            Ok(())
        }
        Err(PersistenceError::Recoverable(PersistenceErrorKind::DecodeFailed { raw, message })) => {
            let policy = controller
                .with_untracked(|controller| controller.decode_policy)
                .map_err(PersistenceError::from)?;
            let default = controller
                .with_untracked(|controller| controller.default.clone())
                .map_err(PersistenceError::from)?;
            let default = default();
            let value_changed = value.get_untracked().map_err(PersistenceError::from)? != default;
            apply_external_runtime_snapshot(controller, None).map_err(PersistenceError::from)?;
            state
                .set(PersistenceState::DecodeError(DecodeErrorInfo {
                    raw: raw.clone(),
                    message: message.clone(),
                }))
                .map_err(PersistenceError::from)?;
            if value_changed {
                value.set(default).map_err(PersistenceError::from)?;
            }
            if matches!(policy, DecodePolicy::RemoveAndUseDefault) {
                let (key, remove_backend) = controller
                    .with_untracked(|controller| {
                        (controller.key.clone(), controller.backend_remove.clone())
                    })
                    .map_err(PersistenceError::from)?;
                let result = remove_backend(&key);
                if let Err(error) = result {
                    state
                        .set(PersistenceState::WriteError(error.message()))
                        .map_err(PersistenceError::from)?;
                    return Err(error);
                }
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn apply_remove_policy<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: Signal<'scope, T>,
    state: Signal<'scope, PersistenceState>,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    invalidate_debounce(controller).map_err(PersistenceError::from)?;
    let policy = controller
        .with_untracked(|controller| controller.remove_policy)
        .map_err(PersistenceError::from)?;
    if !matches!(policy, RemovePolicy::UseDefault) {
        apply_external_runtime_snapshot(controller, None).map_err(PersistenceError::from)?;
        state
            .set(PersistenceState::Ready(String::new()))
            .map_err(PersistenceError::from)?;
        return Ok(());
    }

    let default = controller
        .with_untracked(|controller| controller.default.clone())
        .map_err(PersistenceError::from)?;
    let default = default();
    let value_changed = value.get_untracked().map_err(PersistenceError::from)? != default;
    apply_external_runtime_snapshot(controller, None).map_err(PersistenceError::from)?;
    if value_changed {
        value.set(default).map_err(PersistenceError::from)?;
    }
    state
        .set(PersistenceState::Ready(String::new()))
        .map_err(PersistenceError::from)?;
    Ok(())
}

pub(crate) fn invalidate_debounce<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<()> {
    let timer = controller.update_untracked(|controller| controller.runtime.invalidate())?;
    cancel_timer(timer)?;
    Ok(())
}

fn apply_external_runtime_snapshot<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    raw: Option<String>,
) -> SilexResult<()> {
    let timer = controller.update_untracked(|controller| {
        controller.local_mutation_pending = false;
        controller.runtime.apply_external_snapshot(raw)
    })?;
    cancel_timer(timer)?;
    Ok(())
}

pub(crate) fn take_controller_resources<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<(
    Option<BackendSubscription<'scope>>,
    Option<OwnedTimeout<'scope>>,
)> {
    controller.update_untracked(|controller| {
        let timer = controller.runtime.close();
        controller.local_mutation_pending = false;
        (controller.subscription.take(), timer)
    })
}

pub(crate) fn mark_local_value_write<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<()> {
    controller.update_untracked(|controller| {
        controller.local_mutation_pending = true;
    })
}

pub(crate) fn take_local_mutation<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<bool> {
    controller.update_untracked(|controller| {
        let pending = controller.local_mutation_pending;
        controller.local_mutation_pending = false;
        pending
    })
}

fn set_error_state(state: Signal<'_, PersistenceState>, error: &PersistenceError) {
    let next = match error {
        PersistenceError::Recoverable(PersistenceErrorKind::ReadFailed(message))
        | PersistenceError::Fatal(PersistenceErrorKind::ReadFailed(message)) => {
            PersistenceState::ReadError(message.clone())
        }
        PersistenceError::Recoverable(PersistenceErrorKind::DecodeFailed { raw, message })
        | PersistenceError::Fatal(PersistenceErrorKind::DecodeFailed { raw, message }) => {
            PersistenceState::DecodeError(DecodeErrorInfo {
                raw: raw.clone(),
                message: message.clone(),
            })
        }
        PersistenceError::Recoverable(PersistenceErrorKind::BackendUnavailable)
        | PersistenceError::Fatal(PersistenceErrorKind::BackendUnavailable) => {
            PersistenceState::Unavailable
        }
        _ => PersistenceState::WriteError(error.message()),
    };
    let _ = state.write_signal().set(next);
}
