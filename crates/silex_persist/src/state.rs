use crate::backend::BackendEvent;
use crate::builder::PersistentBuilder;
use crate::{DecodePolicy, NoBackend, NoCodec, PersistenceError, RemovePolicy};
use ref_str::LocalStaticRefStr;
use silex_core::{
    ReactiveError, ReactiveResult, Rx, RxGet, Scope,
    reactivity::{PromotionPlan, ReactiveSource, ReadSignal, RwSignal, StoredValue},
    traits::{RxBase, RxCloneData, RxData, RxRead, RxValue, RxWrite},
};
use silex_dom::attribute::PendingAttribute;
use silex_dom::helpers::TimeoutHandle;
use silex_dom::view::{ApplyAttributes, View, ViewOwner};
use std::cell::RefCell;
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

pub(crate) struct ScopedDebounceState {
    pending: bool,
    generation: u64,
    timer: Option<TimeoutHandle>,
}

impl ScopedDebounceState {
    pub(crate) fn new() -> Self {
        Self {
            pending: false,
            generation: 0,
            timer: None,
        }
    }

    pub(crate) fn begin(&mut self) -> u64 {
        self.invalidate();
        self.pending = true;
        self.generation
    }

    pub(crate) fn set_timer(&mut self, generation: u64, timer: TimeoutHandle) -> bool {
        if self.pending && self.generation == generation {
            self.timer = Some(timer);
            true
        } else {
            timer.clear();
            false
        }
    }

    pub(crate) fn take_ready(&mut self, generation: u64) -> bool {
        if !self.pending || self.generation != generation {
            return false;
        }
        self.pending = false;
        self.timer = None;
        true
    }

    pub(crate) fn invalidate(&mut self) {
        if let Some(timer) = self.timer.take() {
            timer.clear();
        }
        self.pending = false;
        self.generation = self.generation.wrapping_add(1);
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
    pub debounce: Option<Rc<RefCell<ScopedDebounceState>>>,
}

pub struct Persistent<'scope, T> {
    pub(crate) scope: Scope<'scope>,
    pub(crate) value: RwSignal<'scope, T>,
    pub(crate) state: RwSignal<'scope, PersistenceState>,
    pub(crate) controller: StoredValue<'scope, PersistenceController<'scope, T>>,
}

impl<'scope> Persistent<'scope, ()> {
    /// Starts a new persistent binding builder for the given backend key.
    pub fn builder(
        scope: Scope<'scope>,
        key: impl Into<LocalStaticRefStr>,
    ) -> PersistentBuilder<'scope, NoBackend, NoCodec> {
        PersistentBuilder::new(scope, key)
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
    pub fn get(&self) -> T {
        self.value.get()
    }

    pub fn get_untracked(&self) -> T {
        self.value.get_untracked()
    }

    pub fn set(&self, value: T) {
        self.write_value(|current| *current = value);
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.write_value(f);
    }

    pub fn try_set(&self, value: T) -> Result<(), PersistenceError> {
        self.try_update(|current| *current = value)
    }

    pub fn try_update<U>(&self, f: impl FnOnce(&mut T) -> U) -> Result<U, PersistenceError> {
        mark_local_value_write(self.controller)?;
        self.value
            .write_signal()
            .try_update(f)
            .map_err(PersistenceError::from)
    }

    fn write_value(&self, f: impl FnOnce(&mut T)) {
        if let Err(error) = self.try_update(f) {
            panic!("更新 persistent value 失败: {error:?}");
        }
    }

    fn validate_owner(&self) -> Result<(), PersistenceError> {
        if self.scope.is_active() {
            Ok(())
        } else {
            Err(PersistenceError::Reactivity(ReactiveError::NoSuchNode))
        }
    }

    pub fn state(&self) -> ReadSignal<'scope, PersistenceState> {
        self.state.read_signal()
    }

    pub fn key(&self) -> String {
        self.controller
            .with_untracked(|controller| controller.key.to_string())
    }

    /// Return whether the backend supplied or accepted a persisted value.
    ///
    /// This is distinct from [`get_untracked`](Self::get_untracked): the latter
    /// intentionally returns the configured default when storage is empty.
    pub fn has_persisted_value(&self) -> bool {
        self.controller
            .with_untracked(|controller| controller.last_flushed_raw.is_some())
    }

    pub fn reset(&self) {
        let default = match self
            .controller
            .try_with(|controller| controller.default.clone())
        {
            Ok(default) => default,
            Err(ReactiveError::NoSuchNode) => return,
            Err(error) => panic!("读取 persistent default 失败: {error}"),
        };
        if let Err(error) = self.try_set(default())
            && !matches!(
                error,
                PersistenceError::Reactivity(ReactiveError::NoSuchNode)
            )
        {
            panic!("重置 persistent value 失败: {error:?}");
        }
    }

    pub fn remove(&self) -> Result<(), PersistenceError> {
        self.validate_owner()?;
        invalidate_debounce(self.controller);
        let key = self.key();
        let remove_backend = self
            .controller
            .with_untracked(|controller| controller.backend_remove.clone());
        let result = remove_backend(&key);
        match result {
            Ok(()) => {
                let _ = self.controller.try_update_untracked(|controller| {
                    controller.last_flushed_raw = None;
                    clear_external_value_markers(controller);
                });
                self.state.set(PersistenceState::Ready(String::new()));
                Ok(())
            }
            Err(err) => {
                self.state.set(PersistenceState::WriteError(err.message()));
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

impl<'scope, T: RxData> RxBase for Persistent<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        self.value.try_track()
    }
}

impl<'scope, T: RxData> RxRead for Persistent<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> ReactiveResult<U> {
        self.value.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> ReactiveResult<U> {
        self.value.try_with_untracked(f)
    }
}

impl<'scope, T: RxData> RxWrite for Persistent<'scope, T> {
    fn rx_try_update_untracked<U>(
        &self,
        f: impl FnOnce(&mut Self::Value) -> U,
    ) -> ReactiveResult<U> {
        mark_local_value_write(self.controller)?;
        self.value.write_signal().try_update(f)
    }

    fn rx_try_notify(&self) -> ReactiveResult<()> {
        self.value.write_signal().try_notify()
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
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        self.value.into_rx().mount(owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        self.value.into_rx().mount_owned(owner, parent, attrs);
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
    invalidate_debounce(controller);
    clear_external_value_markers_on_controller(controller);
    let current = value
        .read_signal()
        .try_get_untracked()
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
        });
    let should_remove = should_remove(&current);

    if should_remove {
        if last_raw.is_none() {
            let _ = controller.try_update_untracked(|controller| {
                controller.suppress_manual_state_generation = None;
            });
            state.set(PersistenceState::Ready(String::new()));
            return Ok(());
        }
        if let Err(error) = remove_backend(&key) {
            state.set(PersistenceState::WriteError(error.message()));
            return Err(error);
        }
        let _ = controller.try_update_untracked(|controller| {
            controller.last_flushed_raw = None;
            clear_external_value_markers(controller);
        });
        state.set(PersistenceState::Ready(String::new()));
        return Ok(());
    }

    let raw = match encode(&current) {
        Ok(raw) => raw,
        Err(error) => {
            state.set(PersistenceState::WriteError(error.message()));
            return Err(error);
        }
    };

    if last_raw.as_deref() == Some(raw.as_str()) {
        let _ = controller.try_update_untracked(|controller| {
            clear_external_value_markers(controller);
        });
        state.set(PersistenceState::Ready(raw));
        return Ok(());
    }

    if let Err(error) = set_backend(&key, &raw) {
        state.set(PersistenceState::WriteError(error.message()));
        return Err(error);
    }
    let _ = controller.try_update_untracked(|controller| {
        controller.last_flushed_raw = Some(raw.clone());
        clear_external_value_markers(controller);
    });
    state.set(PersistenceState::Ready(raw));
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
        .with_untracked(|controller| (controller.key.clone(), controller.backend_get.clone()));
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
            controller.with_untracked(|controller| controller.key == *key)
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
    invalidate_debounce(controller);
    let decode = controller.with_untracked(|controller| controller.decode.clone());
    let decode_result = decode(&raw);
    match decode_result {
        Ok(decoded) => {
            let value_changed = value.get_untracked() != decoded;
            let _ = controller.try_update_untracked(|controller| {
                controller.last_flushed_raw = Some(raw.clone());
                clear_external_value_markers(controller);
                if value_changed {
                    controller.value_generation = controller.value_generation.wrapping_add(1);
                }
            });
            if value_changed {
                value.set(decoded);
            }
            state.set(PersistenceState::Ready(raw));
            Ok(())
        }
        Err(PersistenceError::DecodeFailed { raw, message }) => {
            let policy = controller.with_untracked(|controller| controller.decode_policy);
            let default = controller.with_untracked(|controller| controller.default.clone());
            let default = default();
            let value_changed = value.get_untracked() != default;
            if value_changed {
                arm_external_value_change(controller);
            } else {
                clear_external_value_markers_on_controller(controller);
            }
            let _ = controller.try_update_untracked(|controller| {
                controller.last_flushed_raw = None;
            });
            state.set(PersistenceState::DecodeError(DecodeErrorInfo {
                raw: raw.clone(),
                message: message.clone(),
            }));
            if value_changed {
                value.set(default);
            }
            if matches!(policy, DecodePolicy::RemoveAndUseDefault) {
                let (key, remove_backend) = controller.with_untracked(|controller| {
                    (controller.key.clone(), controller.backend_remove.clone())
                });
                let result = remove_backend(&key);
                if let Err(error) = result {
                    state.set(PersistenceState::WriteError(error.message()));
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
    invalidate_debounce(controller);
    let policy = controller.with_untracked(|controller| controller.remove_policy);
    if !matches!(policy, RemovePolicy::UseDefault) {
        let _ = controller.try_update_untracked(|controller| {
            controller.last_flushed_raw = None;
            clear_external_value_markers(controller);
        });
        state.set(PersistenceState::Ready(String::new()));
        return Ok(());
    }

    let default = controller.with_untracked(|controller| controller.default.clone());
    let default = default();
    let value_changed = value.get_untracked() != default;
    let _ = controller.try_update_untracked(|controller| {
        controller.last_flushed_raw = None;
        if value_changed {
            let generation = controller.value_generation.wrapping_add(1);
            controller.value_generation = generation;
            controller.skip_next_auto_flush_generation = Some(generation);
            controller.suppress_manual_state_generation = Some(generation);
        } else {
            clear_external_value_markers(controller);
        }
    });
    if value_changed {
        value.set(default);
    }
    state.set(PersistenceState::Ready(String::new()));
    Ok(())
}

fn invalidate_debounce<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) {
    let debounce = controller.with_untracked(|controller| controller.debounce.clone());
    if let Some(debounce) = debounce {
        debounce.borrow_mut().invalidate();
    }
}

pub(crate) fn mark_local_value_write<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> ReactiveResult<()> {
    controller
        .try_update(|controller| {
            controller.value_generation = controller.value_generation.wrapping_add(1);
        })
        .map(|_| ())
}

pub(crate) fn take_skip_next_auto_flush<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> bool {
    controller
        .try_update_untracked(|controller| {
            let should_skip =
                controller.skip_next_auto_flush_generation == Some(controller.value_generation);
            controller.skip_next_auto_flush_generation = None;
            should_skip
        })
        .unwrap_or(false)
}

pub(crate) fn take_suppress_manual_state<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) -> bool {
    controller
        .try_update_untracked(|controller| {
            let should_suppress =
                controller.suppress_manual_state_generation == Some(controller.value_generation);
            controller.suppress_manual_state_generation = None;
            should_suppress
        })
        .unwrap_or(false)
}

fn arm_external_value_change<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) {
    let _ = controller.try_update_untracked(|controller| {
        let generation = controller.value_generation.wrapping_add(1);
        controller.value_generation = generation;
        controller.skip_next_auto_flush_generation = Some(generation);
        controller.suppress_manual_state_generation = Some(generation);
    });
}

fn clear_external_value_markers_on_controller<'scope, T>(
    controller: StoredValue<'scope, PersistenceController<'scope, T>>,
) {
    let _ = controller.try_update_untracked(clear_external_value_markers);
}

fn clear_external_value_markers<T>(controller: &mut PersistenceController<'_, T>) {
    controller.skip_next_auto_flush_generation = None;
    controller.suppress_manual_state_generation = None;
}

fn set_error_state(state: RwSignal<'_, PersistenceState>, error: &PersistenceError) {
    let next = match error {
        PersistenceError::ReadFailed(message) => PersistenceState::ReadError(message.clone()),
        PersistenceError::DecodeFailed { raw, message } => {
            PersistenceState::DecodeError(DecodeErrorInfo {
                raw: raw.clone(),
                message: message.clone(),
            })
        }
        PersistenceError::BackendUnavailable => PersistenceState::Unavailable,
        _ => PersistenceState::WriteError(error.message()),
    };
    let _ = state.write_signal().try_set(next);
}
