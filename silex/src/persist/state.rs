use crate::persist::backend::{BackendEvent, BackendSubscription};
use crate::persist::builder::PersistentBuilder;
use crate::persist::{DecodePolicy, NoBackend, NoCodec, PersistenceError, RemovePolicy};
use silex_core::reactivity::{ReadSignal, RwSignal, StoredValue};
use silex_core::traits::{
    IntoRx, IntoSignal, RxBase, RxData, RxGet, RxInternal, RxRead, RxValue, RxWrite,
};
use silex_core::{Rx, RxValueKind};
use silex_dom::view::{ApplyAttributes, View};
use std::rc::Rc;

pub type PersistenceGetFn = Rc<dyn Fn(&str) -> Result<Option<String>, PersistenceError>>;
pub type PersistenceSetFn = Rc<dyn Fn(&str, &str) -> Result<(), PersistenceError>>;
pub type PersistenceRemoveFn = Rc<dyn Fn(&str) -> Result<(), PersistenceError>>;
pub type PersistenceEncodeFn<T> = Rc<dyn Fn(&T) -> Result<String, PersistenceError>>;
pub type PersistenceDecodeFn<T> = Rc<dyn Fn(&str) -> Result<T, PersistenceError>>;

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

pub(crate) struct PersistenceController<T> {
    pub key: String,
    pub default: Rc<dyn Fn() -> T>,
    pub decode_policy: DecodePolicy,
    pub remove_policy: RemovePolicy,
    pub last_flushed_raw: Option<String>,
    pub skip_next_auto_flush: bool,
    pub backend_get: PersistenceGetFn,
    pub backend_set: PersistenceSetFn,
    pub backend_remove: PersistenceRemoveFn,
    pub encode: PersistenceEncodeFn<T>,
    pub decode: PersistenceDecodeFn<T>,
    pub should_remove: Rc<dyn Fn(&T) -> bool>,
    pub subscription: Option<BackendSubscription>,
}

pub struct Persistent<T> {
    pub(crate) value: RwSignal<T>,
    pub(crate) state: RwSignal<PersistenceState>,
    pub(crate) controller: StoredValue<PersistenceController<T>>,
}

impl Persistent<()> {
    /// Starts a new persistent binding builder for the given backend key.
    ///
    /// This is the entry point for creating any persistent state (LocalStorage, SessionStorage, or URL Query).
    pub fn builder(key: impl Into<String>) -> PersistentBuilder<NoBackend, NoCodec> {
        PersistentBuilder::new(key)
    }
}

impl<T> Clone for Persistent<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Persistent<T> {}

impl<T> Persistent<T>
where
    T: Clone + PartialEq + 'static,
{
    /// Returns the current decoded value and tracks reactive dependencies.
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
    /// assert_eq!(theme.get(), "Light".to_string());
    /// ```
    pub fn get(&self) -> T {
        self.value.get()
    }

    /// Returns the current decoded value without tracking dependencies.
    pub fn get_untracked(&self) -> T {
        self.value.get_untracked()
    }

    /// Replaces the current value.
    ///
    /// In `PersistMode::Immediate`, this also schedules a backend write.
    pub fn set(&self, value: T) {
        self.value.set(value);
    }

    /// Mutates the current value in place.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.value.update(f);
    }

    /// Exposes the inner `RwSignal<T>` for APIs that explicitly require a signal type.
    pub fn signal(&self) -> RwSignal<T> {
        self.value
    }

    /// Returns the current persistence status signal.
    pub fn state(&self) -> ReadSignal<PersistenceState> {
        self.state.read_signal()
    }

    /// Returns the backend key used by this persistent binding.
    pub fn key(&self) -> String {
        self.controller
            .with_untracked(|controller| controller.key.clone())
    }

    /// Resets the in-memory value back to its configured default.
    pub fn reset(&self) {
        let default = self
            .controller
            .with_untracked(|controller| (controller.default)());
        self.value.set(default);
    }

    /// Removes the backend entry for this binding.
    pub fn remove(&self) -> Result<(), PersistenceError> {
        let key = self.key();
        let result = self
            .controller
            .with_untracked(|controller| (controller.backend_remove)(&key));
        match result {
            Ok(()) => {
                let _ = self.controller.try_update_untracked(|controller| {
                    controller.last_flushed_raw = None;
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
        reload_persistent(self.controller, self.value, self.state)
    }

    /// Forces the current in-memory value to be written to the backend.
    ///
    /// This is most useful when the builder was configured with `PersistMode::Manual`.
    ///
    /// ```rust,no_run
    /// use silex::prelude::*;
    ///
    /// let draft = Persistent::builder("draft")
    ///     .local()
    ///     .string()
    ///     .mode(PersistMode::Manual)
    ///     .default(String::new())
    ///     .build();
    ///
    /// draft.set("hello".to_string());
    /// let _ = draft.flush();
    /// ```
    pub fn flush(&self) -> Result<(), PersistenceError> {
        flush_persistent_value(self.controller, self.value, self.state)
    }
}

impl<T: RxData> RxValue for Persistent<T> {
    type Value = T;
}

impl<T: RxData> RxBase for Persistent<T> {
    fn id(&self) -> Option<silex_core::reactivity::NodeId> {
        self.value.id()
    }

    fn track(&self) {
        self.value.track();
    }

    fn is_disposed(&self) -> bool {
        self.value.is_disposed()
    }

    fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        self.value.defined_at()
    }

    fn debug_name(&self) -> Option<String> {
        self.value.debug_name()
    }
}

impl<T: silex_core::traits::RxCloneData> IntoRx for Persistent<T> {
    type RxType = Rx<T, RxValueKind>;

    fn into_rx(self) -> Self::RxType {
        self.value.into_rx()
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<T: silex_core::traits::RxCloneData> IntoSignal for Persistent<T> {
    fn into_signal(self) -> silex_core::reactivity::Signal<T> {
        self.value.into_signal()
    }
}

impl<T: RxData> RxInternal for Persistent<T> {
    type ReadOutput<'a>
        = <RwSignal<T> as RxInternal>::ReadOutput<'a>
    where
        Self: 'a;

    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.value.rx_read_untracked()
    }

    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.value.rx_try_with_untracked(fun)
    }

    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.value.rx_get_adaptive()
    }

    fn rx_is_constant(&self) -> bool {
        false
    }
}

impl<T: 'static> RxWrite for Persistent<T> {
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        self.value.rx_try_update_untracked(fun)
    }

    fn rx_notify(&self) {
        self.value.rx_notify();
    }
}

impl<T: Clone + PartialEq + 'static> From<Persistent<T>> for RwSignal<T> {
    fn from(value: Persistent<T>) -> Self {
        value.signal()
    }
}

impl<T> ApplyAttributes for Persistent<T>
where
    T: silex_core::traits::RxCloneData + Sized + 'static,
    Rx<T, RxValueKind>: ApplyAttributes,
{
}

impl<T> View for Persistent<T>
where
    T: silex_core::traits::RxCloneData + Sized + 'static,
    Rx<T, RxValueKind>: View,
{
    fn mount(&self, parent: &web_sys::Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        (*self).mount_owned(parent, attrs);
    }

    fn mount_owned(self, parent: &web_sys::Node, attrs: Vec<silex_dom::attribute::PendingAttribute>)
    where
        Self: Sized,
    {
        self.into_rx().mount_owned(parent, attrs);
    }
}

pub(crate) fn flush_persistent_value<T: Clone + PartialEq + 'static>(
    controller: StoredValue<PersistenceController<T>>,
    value: RwSignal<T>,
    state: RwSignal<PersistenceState>,
) -> Result<(), PersistenceError> {
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

        if let Err(err) = remove_backend(&key) {
            state.set(PersistenceState::WriteError(err.message()));
            return Err(err);
        }

        let _ = controller.try_update_untracked(|controller| {
            controller.last_flushed_raw = None;
        });
        state.set(PersistenceState::Ready(String::new()));
        return Ok(());
    }

    let raw = match raw {
        Ok(raw) => raw,
        Err(err) => {
            state.set(PersistenceState::WriteError(err.message()));
            return Err(err);
        }
    };

    if last_raw.as_deref() == Some(raw.as_str()) {
        state.set(PersistenceState::Ready(raw));
        return Ok(());
    }

    if let Err(err) = set_backend(&key, &raw) {
        state.set(PersistenceState::WriteError(err.message()));
        return Err(err);
    }
    let _ = controller.try_update_untracked(|controller| {
        controller.last_flushed_raw = Some(raw.clone());
    });
    state.set(PersistenceState::Ready(raw));
    Ok(())
}

pub(crate) fn reload_persistent<T: Clone + PartialEq + 'static>(
    controller: StoredValue<PersistenceController<T>>,
    value: RwSignal<T>,
    state: RwSignal<PersistenceState>,
) -> Result<(), PersistenceError> {
    let key = controller.with_untracked(|controller| controller.key.clone());
    let raw = controller.with_untracked(|controller| (controller.backend_get)(&key))?;
    apply_backend_snapshot(controller, value, state, raw)
}

pub(crate) fn apply_backend_event<T: Clone + PartialEq + 'static>(
    controller: StoredValue<PersistenceController<T>>,
    value: RwSignal<T>,
    state: RwSignal<PersistenceState>,
    event: BackendEvent,
) {
    let result = match event {
        BackendEvent::Set { value: raw, .. } => apply_raw_value(controller, value, state, raw),
        BackendEvent::Removed { .. } => apply_remove_policy(controller, value, state),
        BackendEvent::ExternalRefresh => reload_persistent(controller, value, state),
    };

    if let Err(err) = result {
        match err {
            PersistenceError::ReadFailed(message) => {
                state.set(PersistenceState::ReadError(message))
            }
            _ => state.set(PersistenceState::WriteError(err.message())),
        }
    }
}

fn apply_backend_snapshot<T: Clone + PartialEq + 'static>(
    controller: StoredValue<PersistenceController<T>>,
    value: RwSignal<T>,
    state: RwSignal<PersistenceState>,
    raw: Option<String>,
) -> Result<(), PersistenceError> {
    match raw {
        Some(raw) => apply_raw_value(controller, value, state, raw),
        None => apply_remove_policy(controller, value, state),
    }
}

fn apply_raw_value<T: Clone + PartialEq + 'static>(
    controller: StoredValue<PersistenceController<T>>,
    value: RwSignal<T>,
    state: RwSignal<PersistenceState>,
    raw: String,
) -> Result<(), PersistenceError> {
    let decode_result = controller.with_untracked(|controller| (controller.decode)(&raw));
    match decode_result {
        Ok(decoded) => {
            if value.get_untracked() != decoded {
                value.set(decoded);
            }
            let _ = controller.try_update_untracked(|controller| {
                controller.last_flushed_raw = Some(raw.clone());
            });
            state.set(PersistenceState::Ready(raw));
            Ok(())
        }
        Err(PersistenceError::DecodeFailed { raw, message }) => {
            let policy = controller.with_untracked(|controller| controller.decode_policy);
            state.set(PersistenceState::DecodeError(DecodeErrorInfo {
                raw: raw.clone(),
                message: message.clone(),
            }));
            let default = controller.with_untracked(|controller| (controller.default)());
            value.set(default);
            if matches!(policy, DecodePolicy::RemoveAndUseDefault) {
                let key = controller.with_untracked(|controller| controller.key.clone());
                controller.with_untracked(|controller| (controller.backend_remove)(&key))?;
                let _ = controller.try_update_untracked(|controller| {
                    controller.last_flushed_raw = None;
                });
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn apply_remove_policy<T: Clone + PartialEq + 'static>(
    controller: StoredValue<PersistenceController<T>>,
    value: RwSignal<T>,
    state: RwSignal<PersistenceState>,
) -> Result<(), PersistenceError> {
    let policy = controller.with_untracked(|controller| controller.remove_policy);
    let _ = controller.try_update_untracked(|controller| {
        controller.last_flushed_raw = None;
        controller.skip_next_auto_flush = true;
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
