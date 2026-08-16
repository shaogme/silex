use crate::backend::{BackendEvent, BackendSubscription};
use crate::builder::PersistentBuilder;
use crate::{
    DecodePolicy, NoBackend, NoCodec, NoDefault, PersistenceError, PersistenceErrorKind,
    RemovePolicy,
};
use ref_str::LocalStaticRefStr;
use silex_core::{
    ErrorHandlerInput, OwnerAccess, ReactiveError, Rx, RxGet, SilexErrorKind, SilexResult,
    StoreField,
    reactivity::{PromotionPlan, ReactiveSource, ReadSignal, RwSignal, StoredValue},
    traits::{RxCloneData, RxData, RxRead, RxValue, RxWrite},
};
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{
    ApplyAttributes, HostResourceHandle, MountErrorHandler, MountInstance, MountOwner, View,
};
use std::rc::Rc;
use web_sys::Node;

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

#[derive(Clone, Debug, PartialEq)]
pub enum PersistenceState {
    Ready(String),
    Dirty(String),
    Syncing(String),
    Unavailable,
    ReadError(String),
    DecodeError(DecodeErrorInfo),
    WriteError(String),
}

pub(crate) struct OwnerDebounceState<'scope> {
    pending: bool,
    generation: u64,
    timer: Option<HostResourceHandle<'scope>>,
}

impl<'scope> OwnerDebounceState<'scope> {
    pub(crate) fn new() -> Self {
        Self {
            pending: false,
            generation: 0,
            timer: None,
        }
    }

    pub(crate) fn begin_with_previous_timer(
        &mut self,
    ) -> (u64, Option<HostResourceHandle<'scope>>) {
        let timer = self.timer.take();
        self.pending = true;
        self.generation = self.generation.wrapping_add(1);
        (self.generation, timer)
    }

    pub(crate) fn set_timer(
        &mut self,
        generation: u64,
        timer: HostResourceHandle<'scope>,
    ) -> Option<HostResourceHandle<'scope>> {
        if self.pending && self.generation == generation {
            self.timer = Some(timer);
            None
        } else {
            Some(timer)
        }
    }

    pub(crate) fn take_ready(&mut self, generation: u64) -> bool {
        if !self.pending || self.generation != generation {
            return false;
        }
        self.pending = false;
        if let Some(timer) = self.timer.take() {
            timer.finish();
        }
        true
    }

    pub(crate) fn invalidate(&mut self) -> Option<HostResourceHandle<'scope>> {
        let timer = self.timer.take();
        self.pending = false;
        self.generation = self.generation.wrapping_add(1);
        timer
    }
}

pub(crate) struct PersistenceController<'scope, T: 'scope> {
    pub key: LocalStaticRefStr,
    pub default: Rc<dyn Fn() -> T + 'scope>,
    pub decode_policy: DecodePolicy,
    pub remove_policy: RemovePolicy,
    pub last_flushed_raw: Option<String>,
    pub value_generation: u64,
    pub skip_next_auto_flush_generation: Option<u64>,
    pub suppress_manual_state_generation: Option<u64>,
    pub backend_get: PersistenceGetFn<'scope>,
    pub backend_set: PersistenceSetFn<'scope>,
    pub backend_remove: PersistenceRemoveFn<'scope>,
    pub encode: PersistenceEncodeFn<'scope, T>,
    pub decode: PersistenceDecodeFn<'scope, T>,
    pub should_remove: Rc<dyn Fn(&T) -> bool + 'scope>,
    pub debounce: Option<OwnerDebounceState<'scope>>,
    pub subscription: Option<BackendSubscription<'scope>>,
}

pub struct Persistent<'scope, T> {
    pub(crate) owner: OwnerAccess<'scope>,
    pub(crate) value: RwSignal<'scope, T>,
    pub(crate) state: RwSignal<'scope, PersistenceState>,
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
    pub fn signal(&self) -> RwSignal<'scope, T> {
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
        if self.owner.is_active() {
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
            .with_untracked(|controller| controller.last_flushed_raw.is_some())
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
                self.controller.update_untracked(|controller| {
                    controller.last_flushed_raw = None;
                    clear_external_value_markers(controller);
                })?;
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

    pub fn reload(&self) -> Result<(), PersistenceError> {
        self.validate_owner()?;
        let result = reload_persistent(self.controller, self.value, self.state);
        if let Err(error) = &result {
            set_error_state(self.state, error);
        }
        result
    }

    pub fn flush(&self) -> Result<(), PersistenceError> {
        self.validate_owner()?;
        flush_persistent_value(self.controller, self.value, self.state)
    }
}

impl<'scope, T: RxData> RxValue for Persistent<'scope, T> {
    type Value = T;
}

impl<'scope, T: RxData> RxRead for Persistent<'scope, T> {
    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
        self.value.with(f)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
        self.value.with_untracked(f)
    }
}

impl<'scope, T: RxData> RxWrite for Persistent<'scope, T> {
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

impl<'scope, T: 'scope> From<Persistent<'scope, T>> for RwSignal<'scope, T> {
    fn from(value: Persistent<'scope, T>) -> Self {
        value.signal()
    }
}

impl<'scope, T> ApplyAttributes<'scope> for Persistent<'scope, T>
where
    T: RxCloneData + 'scope,
    Rx<'scope, T>: ApplyAttributes<'scope>,
{
}

impl<'scope, T> View<'scope> for Persistent<'scope, T>
where
    T: RxCloneData + 'scope,
    Rx<'scope, T>: View<'scope>,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        self.value
            .into_rx()
            .mount(owner, parent, attrs, error_handler)
    }
}

pub(crate) fn flush_persistent_value<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: RwSignal<'scope, T>,
    state: RwSignal<'scope, PersistenceState>,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    invalidate_debounce(controller).map_err(PersistenceError::from)?;
    clear_external_value_markers_on_controller(controller).map_err(PersistenceError::from)?;
    let current = value
        .read_signal()
        .get_untracked()
        .map_err(PersistenceError::from)?;
    let (key, last_raw, set_backend, remove_backend, should_remove, encode) = controller
        .with_untracked(|controller| {
            (
                controller.key.clone(),
                controller.last_flushed_raw.clone(),
                controller.backend_set.clone(),
                controller.backend_remove.clone(),
                controller.should_remove.clone(),
                controller.encode.clone(),
            )
        })
        .map_err(PersistenceError::from)?;
    let should_remove = should_remove(&current);

    if should_remove {
        if last_raw.is_none() {
            controller
                .update_untracked(|controller| {
                    controller.suppress_manual_state_generation = None;
                })
                .map_err(PersistenceError::from)?;
            state
                .set(PersistenceState::Ready(String::new()))
                .map_err(PersistenceError::from)?;
            return Ok(());
        }
        if let Err(error) = remove_backend(&key) {
            state
                .set(PersistenceState::WriteError(error.message()))
                .map_err(PersistenceError::from)?;
            return Err(error);
        }
        controller
            .update_untracked(|controller| {
                controller.last_flushed_raw = None;
                clear_external_value_markers(controller);
            })
            .map_err(PersistenceError::from)?;
        state
            .set(PersistenceState::Ready(String::new()))
            .map_err(PersistenceError::from)?;
        return Ok(());
    }

    let raw = match encode(&current) {
        Ok(raw) => raw,
        Err(error) => {
            state
                .set(PersistenceState::WriteError(error.message()))
                .map_err(PersistenceError::from)?;
            return Err(error);
        }
    };

    if last_raw.as_deref() == Some(raw.as_str()) {
        controller
            .update_untracked(|controller| {
                clear_external_value_markers(controller);
            })
            .map_err(PersistenceError::from)?;
        state
            .set(PersistenceState::Ready(raw))
            .map_err(PersistenceError::from)?;
        return Ok(());
    }

    if let Err(error) = set_backend(&key, &raw) {
        state
            .set(PersistenceState::WriteError(error.message()))
            .map_err(PersistenceError::from)?;
        return Err(error);
    }
    controller
        .update_untracked(|controller| {
            controller.last_flushed_raw = Some(raw.clone());
            clear_external_value_markers(controller);
        })
        .map_err(PersistenceError::from)?;
    state
        .set(PersistenceState::Ready(raw))
        .map_err(PersistenceError::from)?;
    Ok(())
}

pub(crate) fn reload_persistent<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
    value: RwSignal<'scope, T>,
    state: RwSignal<'scope, PersistenceState>,
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
    value: RwSignal<'scope, T>,
    state: RwSignal<'scope, PersistenceState>,
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
    value: RwSignal<'scope, T>,
    state: RwSignal<'scope, PersistenceState>,
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
    value: RwSignal<'scope, T>,
    state: RwSignal<'scope, PersistenceState>,
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
            controller
                .update_untracked(|controller| {
                    controller.last_flushed_raw = Some(raw.clone());
                    clear_external_value_markers(controller);
                    if value_changed {
                        controller.value_generation = controller.value_generation.wrapping_add(1);
                    }
                })
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
            if value_changed {
                arm_external_value_change(controller).map_err(PersistenceError::from)?;
            } else {
                clear_external_value_markers_on_controller(controller)
                    .map_err(PersistenceError::from)?;
            }
            controller
                .update_untracked(|controller| {
                    controller.last_flushed_raw = None;
                })
                .map_err(PersistenceError::from)?;
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
    value: RwSignal<'scope, T>,
    state: RwSignal<'scope, PersistenceState>,
) -> Result<(), PersistenceError>
where
    T: Clone + PartialEq + 'scope,
{
    invalidate_debounce(controller).map_err(PersistenceError::from)?;
    let policy = controller
        .with_untracked(|controller| controller.remove_policy)
        .map_err(PersistenceError::from)?;
    if !matches!(policy, RemovePolicy::UseDefault) {
        controller
            .update_untracked(|controller| {
                controller.last_flushed_raw = None;
                clear_external_value_markers(controller);
            })
            .map_err(PersistenceError::from)?;
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
    controller
        .update_untracked(|controller| {
            controller.last_flushed_raw = None;
            if value_changed {
                let generation = controller.value_generation.wrapping_add(1);
                controller.value_generation = generation;
                controller.skip_next_auto_flush_generation = Some(generation);
                controller.suppress_manual_state_generation = Some(generation);
            } else {
                clear_external_value_markers(controller);
            }
        })
        .map_err(PersistenceError::from)?;
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
    let timer = controller.update_untracked(|controller| {
        controller
            .debounce
            .as_mut()
            .and_then(OwnerDebounceState::invalidate)
    })?;
    if let Some(timer) = &timer {
        timer.cancel();
    }
    drop(timer);
    Ok(())
}

pub(crate) fn take_controller_resources<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<(
    Option<BackendSubscription<'scope>>,
    Option<HostResourceHandle<'scope>>,
)> {
    controller.update_untracked(|controller| {
        let timer = controller
            .debounce
            .as_mut()
            .and_then(OwnerDebounceState::invalidate);
        (controller.subscription.take(), timer)
    })
}

pub(crate) fn mark_local_value_write<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<()> {
    controller.update(|controller| {
        controller.value_generation = controller.value_generation.wrapping_add(1);
    })
}

pub(crate) fn take_skip_next_auto_flush<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<bool> {
    controller.update_untracked(|controller| {
        let should_skip =
            controller.skip_next_auto_flush_generation == Some(controller.value_generation);
        controller.skip_next_auto_flush_generation = None;
        should_skip
    })
}

pub(crate) fn take_suppress_manual_state<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<bool> {
    controller.update_untracked(|controller| {
        let should_suppress =
            controller.suppress_manual_state_generation == Some(controller.value_generation);
        controller.suppress_manual_state_generation = None;
        should_suppress
    })
}

fn arm_external_value_change<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<()> {
    controller.update_untracked(|controller| {
        let generation = controller.value_generation.wrapping_add(1);
        controller.value_generation = generation;
        controller.skip_next_auto_flush_generation = Some(generation);
        controller.suppress_manual_state_generation = Some(generation);
    })
}

fn clear_external_value_markers_on_controller<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> SilexResult<()> {
    controller.update_untracked(clear_external_value_markers)
}

fn clear_external_value_markers<T>(controller: &mut PersistenceController<'_, T>) {
    controller.skip_next_auto_flush_generation = None;
    controller.suppress_manual_state_generation = None;
}

fn set_error_state(state: RwSignal<'_, PersistenceState>, error: &PersistenceError) {
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
