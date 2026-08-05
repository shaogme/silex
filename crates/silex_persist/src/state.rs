use crate::backend::{BackendEvent, BackendSubscription};
use crate::builder::PersistentBuilder;
use crate::{DecodePolicy, NoBackend, NoCodec, PersistenceError, RemovePolicy};
use ref_str::LocalStaticRefStr;
use silex_core::RxGet;
use silex_core::{
    Rx, Scope,
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
    pub skip_next_auto_flush: bool,
    pub suppress_manual_state: bool,
    pub backend_get: PersistenceGetFn<'scope>,
    pub backend_set: PersistenceSetFn<'scope>,
    pub backend_remove: PersistenceRemoveFn<'scope>,
    pub encode: PersistenceEncodeFn<'scope, T>,
    pub decode: PersistenceDecodeFn<'scope, T>,
    pub should_remove: Rc<dyn Fn(&T) -> bool + 'scope>,
    pub subscription: Rc<RefCell<Option<BackendSubscription<'scope>>>>,
    pub debounce: Option<Rc<RefCell<ScopedDebounceState>>>,
}

pub struct Persistent<'scope, T> {
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
        if self.value.is_alive() {
            self.value.set(value);
        }
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        if self.value.is_alive() {
            self.value.update(f);
        }
    }

    pub fn state(&self) -> ReadSignal<'scope, PersistenceState> {
        self.state.read_signal()
    }

    pub fn key(&self) -> String {
        self.controller
            .with_untracked(|controller| controller.key.to_string())
    }

    pub fn reset(&self) {
        if !self.value.is_alive() {
            return;
        }
        let default = self
            .controller
            .with_untracked(|controller| (controller.default)());
        self.value.set(default);
    }

    pub fn remove(&self) -> Result<(), PersistenceError> {
        if !self.value.is_alive() {
            return Err(PersistenceError::InvalidConfiguration(
                "persistent scope is inactive".to_string(),
            ));
        }
        invalidate_debounce(self.controller);
        let key = self.key();
        let result = self
            .controller
            .with_untracked(|controller| (controller.backend_remove)(&key));
        match result {
            Ok(()) => {
                let _ = self.controller.try_update_untracked(|controller| {
                    controller.last_flushed_raw = None;
                    controller.skip_next_auto_flush = true;
                    controller.suppress_manual_state = true;
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
        if !self.value.is_alive() {
            return Err(PersistenceError::InvalidConfiguration(
                "persistent scope is inactive".to_string(),
            ));
        }
        let result = reload_persistent(self.controller, self.value, self.state);
        if let Err(error) = &result {
            set_error_state(self.state, error);
        }
        result
    }

    pub fn flush(&self) -> Result<(), PersistenceError> {
        if !self.value.is_alive() {
            return Err(PersistenceError::InvalidConfiguration(
                "persistent scope is inactive".to_string(),
            ));
        }
        flush_persistent_value(self.controller, self.value, self.state)
    }
}

impl<'scope, T: RxData> RxValue for Persistent<'scope, T> {
    type Value = T;
}

impl<'scope, T: RxData> RxBase for Persistent<'scope, T> {
    fn track(&self) {
        self.value.track();
    }

    fn is_alive(&self) -> bool {
        self.value.is_alive()
    }
}

impl<'scope, T: RxData> RxRead for Persistent<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.value.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.value.try_with_untracked(f)
    }
}

impl<'scope, T: RxData> RxWrite for Persistent<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        self.value.rx_try_update_untracked(f)
    }

    fn rx_notify(&self) {
        self.value.rx_notify();
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
    let current = value.get_untracked();
    let (key, raw, last_raw, set_backend, remove_backend, should_remove) = controller
        .with_untracked(|controller| {
            let should_remove = (controller.should_remove)(&current);
            let raw = if should_remove {
                Ok(String::new())
            } else {
                (controller.encode)(&current)
            };
            (
                controller.key.clone(),
                raw,
                controller.last_flushed_raw.clone(),
                controller.backend_set.clone(),
                controller.backend_remove.clone(),
                should_remove,
            )
        });

    if should_remove {
        if last_raw.is_none() {
            state.set(PersistenceState::Ready(String::new()));
            return Ok(());
        }
        if let Err(error) = remove_backend(&key) {
            state.set(PersistenceState::WriteError(error.message()));
            return Err(error);
        }
        let _ = controller.try_update_untracked(|controller| {
            controller.last_flushed_raw = None;
            controller.suppress_manual_state = false;
        });
        state.set(PersistenceState::Ready(String::new()));
        return Ok(());
    }

    let raw = match raw {
        Ok(raw) => raw,
        Err(error) => {
            state.set(PersistenceState::WriteError(error.message()));
            return Err(error);
        }
    };

    if last_raw.as_deref() == Some(raw.as_str()) {
        let _ = controller.try_update_untracked(|controller| {
            controller.suppress_manual_state = false;
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
        controller.suppress_manual_state = false;
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
    let key = controller.with_untracked(|controller| controller.key.clone());
    let raw = controller.with_untracked(|controller| (controller.backend_get)(&key))?;
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
    let decode_result = controller.with_untracked(|controller| (controller.decode)(&raw));
    match decode_result {
        Ok(decoded) => {
            let _ = controller.try_update_untracked(|controller| {
                controller.last_flushed_raw = Some(raw.clone());
                controller.skip_next_auto_flush = false;
                controller.suppress_manual_state = false;
            });
            if value.get_untracked() != decoded {
                value.set(decoded);
            }
            state.set(PersistenceState::Ready(raw));
            Ok(())
        }
        Err(PersistenceError::DecodeFailed { raw, message }) => {
            let policy = controller.with_untracked(|controller| controller.decode_policy);
            let default = controller.with_untracked(|controller| (controller.default)());
            let _ = controller.try_update_untracked(|controller| {
                controller.last_flushed_raw = None;
                controller.skip_next_auto_flush = true;
                controller.suppress_manual_state = true;
            });
            state.set(PersistenceState::DecodeError(DecodeErrorInfo {
                raw: raw.clone(),
                message: message.clone(),
            }));
            if value.get_untracked() != default {
                value.set(default);
            }
            if matches!(policy, DecodePolicy::RemoveAndUseDefault) {
                let key = controller.with_untracked(|controller| controller.key.clone());
                let result =
                    controller.with_untracked(|controller| (controller.backend_remove)(&key));
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
    let _ = controller.try_update_untracked(|controller| {
        controller.last_flushed_raw = None;
        controller.skip_next_auto_flush = true;
        controller.suppress_manual_state = true;
    });
    if matches!(policy, RemovePolicy::UseDefault) {
        let default = controller.with_untracked(|controller| (controller.default)());
        if value.get_untracked() != default {
            value.set(default);
        }
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

fn set_error_state(state: RwSignal<'_, PersistenceState>, error: &PersistenceError) {
    match error {
        PersistenceError::ReadFailed(message) => {
            state.set(PersistenceState::ReadError(message.clone()))
        }
        PersistenceError::DecodeFailed { raw, message } => {
            state.set(PersistenceState::DecodeError(DecodeErrorInfo {
                raw: raw.clone(),
                message: message.clone(),
            }))
        }
        PersistenceError::BackendUnavailable => state.set(PersistenceState::Unavailable),
        _ => state.set(PersistenceState::WriteError(error.message())),
    }
}
