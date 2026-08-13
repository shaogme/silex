pub mod any;
pub mod list;
pub(crate) mod owner;
pub mod reactive;

pub use any::*;
pub use list::*;
pub use reactive::*;

use crate::attribute::PendingAttribute;
use silex_core::{
    CallbackInvokeError, CleanupError, CompletionOnce, CompletionSender, ErrorReporter, OwnedScope,
    ReactiveError, RuntimeInputs, Rx, Scope, SilexError, SilexErrorKind, SilexResult, StoredValue,
    reactivity::ReactiveSource,
    traits::{RxData, RxValue},
    unwind_safe,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    ops::{Add, Deref, Div, Mul, Sub},
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Node;

pub use owner::RowUpdater;
use owner::{DomRange, RowController, RowControllerConfig, RowRender, RowRenderArgs};

/// Owner capabilities captured by a mounted view or attribute operation.
///
/// The token owns only registration functions. It never stores a borrowed
/// `ViewOwner`, so an effect cannot outlive the adapter stack frame used by
/// the original mount call.
pub type ViewEffect<'scope> = Box<dyn FnMut() -> SilexResult<()> + 'scope>;
pub type ViewCleanup<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;
pub type ViewErrorHandler<'scope> = ErrorReporter<'scope>;
pub(crate) type CleanupReporter = Rc<dyn Fn(CleanupError)>;

#[derive(Clone)]
struct EffectRegistrar<'scope> {
    inner: Rc<dyn EffectRegister<'scope> + 'scope>,
}

trait EffectRegister<'scope> {
    fn register(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>;
}

impl<'scope, F> EffectRegister<'scope> for F
where
    F: Fn(RuntimeInputs, ViewEffect<'scope>, ViewErrorHandler<'scope>) -> SilexResult<()> + 'scope,
{
    fn register(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self(inputs, callback, error_handler)
    }
}

impl<'scope> EffectRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(RuntimeInputs, ViewEffect<'scope>, ViewErrorHandler<'scope>) -> SilexResult<()>
            + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.inner.register(inputs, callback, error_handler)
    }
}

type ValidationCallback<'scope> = Rc<dyn Fn(&RuntimeInputs) -> SilexResult<()> + 'scope>;

#[derive(Clone)]
struct ValidationRegistrar<'scope> {
    inner: ValidationCallback<'scope>,
}

impl<'scope> ValidationRegistrar<'scope> {
    fn new<F>(validate: F) -> Self
    where
        F: Fn(&RuntimeInputs) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(validate),
        }
    }

    fn call(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        (self.inner)(inputs)
    }
}

#[derive(Clone)]
struct CleanupRegistrar<'scope> {
    inner: Rc<dyn CleanupRegister<'scope> + 'scope>,
}

trait CleanupRegister<'scope> {
    fn register(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>;
}

impl<'scope, F> CleanupRegister<'scope> for F
where
    F: Fn(ViewCleanup<'scope>, ViewErrorHandler<'scope>) -> SilexResult<()> + 'scope,
{
    fn register(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self(cleanup, error_handler)
    }
}

impl<'scope> CleanupRegistrar<'scope> {
    fn new<F>(register: F) -> Self
    where
        F: Fn(ViewCleanup<'scope>, ViewErrorHandler<'scope>) -> SilexResult<()> + 'scope,
    {
        Self {
            inner: Rc::new(register),
        }
    }

    fn call(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.inner.register(cleanup, error_handler)
    }
}

#[derive(Clone)]
struct OwnedScopeRegistrar<'scope> {
    inner: Rc<dyn OwnedScopeRegister<'scope> + 'scope>,
}

trait OwnedScopeRegister<'scope> {
    fn create(&self) -> SilexResult<OwnedScope<'scope>>;
}

impl<'scope, F> OwnedScopeRegister<'scope> for F
where
    F: Fn() -> SilexResult<OwnedScope<'scope>> + 'scope,
{
    fn create(&self) -> SilexResult<OwnedScope<'scope>> {
        self()
    }
}

impl<'scope> OwnedScopeRegistrar<'scope> {
    fn new<F>(create: F) -> Self
    where
        F: Fn() -> SilexResult<OwnedScope<'scope>> + 'scope,
    {
        Self {
            inner: Rc::new(create),
        }
    }

    fn call(&self) -> SilexResult<OwnedScope<'scope>> {
        self.inner.create()
    }
}

#[derive(Clone)]
struct ActiveRegistrar<'scope> {
    inner: Rc<dyn Fn() -> bool + 'scope>,
}

impl<'scope> ActiveRegistrar<'scope> {
    fn new<F>(is_active: F) -> Self
    where
        F: Fn() -> bool + 'scope,
    {
        Self {
            inner: Rc::new(is_active),
        }
    }

    fn get(&self) -> bool {
        (self.inner)()
    }
}

#[derive(Clone)]
struct CompletionRegistrar<'scope> {
    sender: CompletionSenderFactory<'scope>,
    once: CompletionOnceFactory<'scope>,
}

type HostCallbackFn<'scope> = Box<dyn FnMut(JsValue) -> SilexResult<()> + 'scope>;
type CompletionSenderFactory<'scope> =
    Rc<dyn Fn(HostCallbackFn<'scope>) -> SilexResult<CompletionSender<JsValue>> + 'scope>;
type CompletionOnceFactory<'scope> =
    Rc<dyn Fn(HostCallbackFn<'scope>) -> SilexResult<CompletionOnce<JsValue>> + 'scope>;

impl<'scope> CompletionRegistrar<'scope> {
    fn new<FS, FO>(sender: FS, once: FO) -> Self
    where
        FS: Fn(HostCallbackFn<'scope>) -> SilexResult<CompletionSender<JsValue>> + 'scope,
        FO: Fn(HostCallbackFn<'scope>) -> SilexResult<CompletionOnce<JsValue>> + 'scope,
    {
        Self {
            sender: Rc::new(sender),
            once: Rc::new(once),
        }
    }

    fn call_sender(
        &self,
        callback: HostCallbackFn<'scope>,
    ) -> SilexResult<CompletionSender<JsValue>> {
        (self.sender)(callback)
    }

    fn call_once(&self, callback: HostCallbackFn<'scope>) -> SilexResult<CompletionOnce<JsValue>> {
        (self.once)(callback)
    }
}

#[derive(Clone)]
enum PreviousEffectOwner<'scope> {
    Scoped(Scope<'scope>),
    Owned(Rc<OwnedScope<'scope>>),
}

impl<'scope> PreviousEffectOwner<'scope> {
    fn register<T, F>(
        &self,
        inputs: RuntimeInputs,
        callback: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        match self {
            Self::Scoped(scope) => scope
                .effect_with_previous_from(inputs, callback, error_handler)
                .map(|_| ()),
            Self::Owned(scope) => scope
                .effect_with_previous_from(inputs, callback, error_handler)
                .map(|_| ()),
        }
    }
}

/// A host resource cancellation handle owned by a view scope.
///
/// The handle deliberately exposes no reactive capability. Cancellation is
/// idempotent and the owner retains a clone so dropping this value early does
/// not transfer lifecycle ownership away from the view.
type ResourceGate = Rc<Cell<bool>>;

/// Shared mutable state used by generated code and host resources.
#[doc(hidden)]
pub struct SharedSlot<T> {
    inner: Rc<RefCell<T>>,
}

impl<T> Clone for SharedSlot<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> SharedSlot<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
        }
    }

    pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> R {
        callback(&self.inner.borrow())
    }

    pub fn with_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
        callback(&mut self.inner.borrow_mut())
    }

    pub fn replace(&self, value: T) -> T {
        self.inner.replace(value)
    }

    pub fn set(&self, value: T) {
        drop(self.replace(value));
    }

    pub fn take(&self) -> T
    where
        T: Default,
    {
        self.replace(T::default())
    }
}

/// Owner-bound mutable state that can only be accessed through closures.
///
/// The state rejects access after its owner becomes inactive. The framework
/// uses the cleanup-only methods while an owner is being disposed so cleanup
/// can still take the final value after the runtime has rejected new work.
/// States created for a lexical `Scope` are backed by that scope's
/// `StoredValue`, while states created for an `OwnedScope` use the local
/// fallback because an owned view owner intentionally cannot create nodes.
pub struct OwnerState<'scope, T> {
    value: OwnerStateValue<'scope, T>,
    active: ActiveRegistrar<'scope>,
}

enum OwnerStateValue<'scope, T> {
    Shared(SharedSlot<Option<T>>),
    Stored(StoredValue<'scope, Option<T>>),
}

impl<'scope, T> Clone for OwnerStateValue<'scope, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Shared(value) => Self::Shared(value.clone()),
            Self::Stored(value) => Self::Stored(*value),
        }
    }
}

impl<'scope, T> Clone for OwnerState<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            active: self.active.clone(),
        }
    }
}

impl<'scope, T: 'scope> OwnerState<'scope, T> {
    fn new(value: T, active: ActiveRegistrar<'scope>) -> Self {
        Self {
            value: OwnerStateValue::Shared(SharedSlot::new(Some(value))),
            active,
        }
    }

    fn new_stored(value: StoredValue<'scope, Option<T>>, active: ActiveRegistrar<'scope>) -> Self {
        Self {
            value: OwnerStateValue::Stored(value),
            active,
        }
    }

    fn ensure_access(&self) -> SilexResult<()> {
        if self.active.get() {
            Ok(())
        } else {
            Err(SilexError::fatal(ReactiveError::NoSuchNode))
        }
    }

    pub fn with<R>(&self, callback: impl FnOnce(&T) -> R) -> SilexResult<R> {
        self.ensure_access()?;
        match &self.value {
            OwnerStateValue::Shared(value) => value.with(|value| {
                value
                    .as_ref()
                    .map(callback)
                    .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
            }),
            OwnerStateValue::Stored(value) => value
                .with(|value| value.as_ref().map(callback))?
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
        }
    }

    pub fn update<R>(&self, callback: impl FnOnce(&mut T) -> R) -> SilexResult<R> {
        self.ensure_access()?;
        match &self.value {
            OwnerStateValue::Shared(value) => value.with_mut(|value| {
                value
                    .as_mut()
                    .map(callback)
                    .ok_or(SilexError::fatal(ReactiveError::NoSuchNode))
            }),
            OwnerStateValue::Stored(value) => value
                .update(|value| value.as_mut().map(callback))
                .map_err(SilexError::fatal)?
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
        }
    }

    pub fn take(&self) -> SilexResult<T> {
        self.ensure_access()?;
        match &self.value {
            OwnerStateValue::Shared(value) => value
                .with_mut(Option::take)
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
            OwnerStateValue::Stored(value) => value
                .update(Option::take)
                .map_err(SilexError::fatal)?
                .ok_or(SilexError::fatal(ReactiveError::NoSuchNode)),
        }
    }

    pub fn replace(&self, value: T) -> SilexResult<Option<T>> {
        self.ensure_access()?;
        match &self.value {
            OwnerStateValue::Shared(current) => {
                Ok(current.with_mut(|current| current.replace(value)))
            }
            OwnerStateValue::Stored(current) => current
                .update(|current| current.replace(value))
                .map_err(SilexError::fatal),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    #[doc(hidden)]
    pub fn take_for_cleanup(&self) -> Option<T> {
        match &self.value {
            OwnerStateValue::Shared(value) => value.with_mut(Option::take),
            OwnerStateValue::Stored(value) => value.update(Option::take).ok().flatten(),
        }
    }
}

type CancelAction<'scope> = SharedSlot<Option<Box<dyn FnOnce() + 'scope>>>;

#[derive(Clone)]
pub(crate) struct JsCallbackResource {
    callback: SharedSlot<Option<JsValue>>,
    active: ResourceGate,
}

impl JsCallbackResource {
    fn new(active: ResourceGate, callback: Option<JsValue>) -> Self {
        Self {
            callback: SharedSlot::new(callback),
            active,
        }
    }

    fn set_callback(&self, callback: JsValue) {
        self.callback.with_mut(|current| {
            debug_assert!(current.is_none());
            *current = Some(callback);
        });
    }

    fn callback_function(&self) -> js_sys::Function {
        self.callback.with(|callback| {
            callback
                .as_ref()
                .expect("host callback is present")
                .unchecked_ref::<js_sys::Function>()
                .clone()
        })
    }

    pub(crate) fn cancel_once(&self) {
        self.active.set(false);
        let _ = self.callback.take();
    }
}

pub struct HostResourceHandle<'scope> {
    cancel: CancelAction<'scope>,
    active: ResourceGate,
    callback: Option<JsCallbackResource>,
}

impl<'scope> Clone for HostResourceHandle<'scope> {
    fn clone(&self) -> Self {
        Self {
            cancel: self.cancel.clone(),
            active: self.active.clone(),
            callback: self.callback.clone(),
        }
    }
}

impl<'scope> HostResourceHandle<'scope> {
    pub(crate) fn inactive() -> Self {
        Self::with_parts(Rc::new(Cell::new(false)), None, None)
    }

    fn with_gate<F>(active: ResourceGate, cancel: F) -> Self
    where
        F: FnOnce() + 'scope,
    {
        Self::with_parts(active, Some(Box::new(cancel)), None)
    }

    pub(crate) fn from_js_callback(callback: &HostCallback, value: JsValue) -> Self {
        Self::with_parts(
            callback.gate.clone(),
            None,
            Some(JsCallbackResource::new(callback.gate.clone(), Some(value))),
        )
    }

    pub(crate) fn empty_js_callback(callback: &HostCallback) -> Self {
        Self::with_parts(
            callback.gate.clone(),
            None,
            Some(JsCallbackResource::new(callback.gate.clone(), None)),
        )
    }

    fn with_parts(
        active: ResourceGate,
        cancel: Option<Box<dyn FnOnce() + 'scope>>,
        callback: Option<JsCallbackResource>,
    ) -> Self {
        Self {
            cancel: SharedSlot::new(cancel),
            active,
            callback,
        }
    }

    pub(crate) fn set_js_callback(&self, callback: JsValue) {
        self.callback_resource().set_callback(callback);
    }

    pub(crate) fn js_callback_function(&self) -> js_sys::Function {
        self.callback_resource().callback_function()
    }

    pub(crate) fn callback_resource(&self) -> JsCallbackResource {
        self.callback
            .clone()
            .expect("host callback resource is present")
    }

    fn install_cancel<F>(&self, cancel: F)
    where
        F: FnOnce() + 'scope,
    {
        self.cancel.with_mut(|current| {
            debug_assert!(current.is_none());
            *current = Some(Box::new(cancel));
        });
    }

    pub(crate) fn cancel_once(&self) {
        let was_active = self.active.replace(false);
        if let Some(callback) = &self.callback {
            callback.cancel_once();
        }
        let cancel = self.cancel.take();
        if was_active && let Some(cancel) = cancel {
            cancel();
        }
    }

    /// Cancel the host resource. Repeated calls are harmless.
    pub fn cancel(&self) {
        self.cancel_once();
    }

    /// Mark a one-shot host resource as completed without running its physical
    /// cancellation action.
    pub fn finish(&self) {
        self.active.set(false);
        if let Some(callback) = &self.callback {
            callback.cancel_once();
        }
        let _ = self.cancel.take();
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }
}

impl Drop for HostResourceHandle<'_> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.cancel.inner) == 1 {
            self.cancel_once();
        }
    }
}

#[derive(Clone)]
enum HostDestination {
    Once(CompletionOnce<JsValue>),
    Sender(CompletionSender<JsValue>),
}

impl HostDestination {
    fn dispatch(&self, payload: JsValue) -> Result<bool, CallbackInvokeError<SilexError>> {
        match self {
            Self::Once(destination) => destination.submit(payload),
            Self::Sender(destination) => destination.submit(payload),
        }
    }

    fn cancel(&self) {
        match self {
            Self::Once(destination) => destination.cancel(),
            Self::Sender(destination) => destination.cancel(),
        }
    }
}

/// A `'static` browser closure's only path back into a scoped view.
#[derive(Clone)]
pub(crate) struct HostCallback {
    destination: HostDestination,
    gate: ResourceGate,
    error_handler: ErrorReporter<'static>,
}

// HostCallback only carries framework-owned gate and Completion state. User
// callbacks are invoked behind Completion's panic cleanup boundary.
impl UnwindSafe for HostCallback {}

impl HostCallback {
    fn report_error(&self, error: SilexError) {
        let handler_result = catch_unwind(AssertUnwindSafe(|| self.error_handler.handle(error)));
        if let Err(handler_panic) = handler_result {
            let _ = catch_unwind(AssertUnwindSafe(|| self.cancel()));
            resume_unwind(handler_panic);
        }
    }

    pub(crate) fn dispatch(&self, payload: JsValue) -> bool {
        if !self.gate.get() {
            return false;
        }
        match self.destination.dispatch(payload) {
            Ok(active) => active,
            Err(CallbackInvokeError::User(error)) => {
                self.report_error(error);
                self.gate.get()
            }
            Err(CallbackInvokeError::Runtime(error)) => {
                self.report_error(SilexError::fatal(error));
                self.gate.get()
            }
        }
    }

    pub(crate) fn finish(&self) {
        self.gate.set(false);
    }

    pub(crate) fn cancel(&self) {
        self.gate.set(false);
        self.destination.cancel();
    }
}

fn erase_error_handler<'scope>(handler: ErrorReporter<'scope>) -> ErrorReporter<'static> {
    // SAFETY: HostCallback's gate is cleared by owner cleanup before the scoped
    // storage can be dropped. Dispatch checks the gate before touching this key.
    unsafe { std::mem::transmute(handler) }
}

#[derive(Clone)]
pub struct ViewOwnerToken<'scope> {
    effect: EffectRegistrar<'scope>,
    previous_effect: PreviousEffectOwner<'scope>,
    validate: ValidationRegistrar<'scope>,
    cleanup: CleanupRegistrar<'scope>,
    owned_scope: OwnedScopeRegistrar<'scope>,
    completion: CompletionRegistrar<'scope>,
    active: ActiveRegistrar<'scope>,
    state_scope: Option<Scope<'scope>>,
    cleanup_reporter: Option<CleanupReporter>,
}

struct ViewOwnerTokenParts<'scope> {
    effect: EffectRegistrar<'scope>,
    previous_effect: PreviousEffectOwner<'scope>,
    validate: ValidationRegistrar<'scope>,
    cleanup: CleanupRegistrar<'scope>,
    owned_scope: OwnedScopeRegistrar<'scope>,
    completion: CompletionRegistrar<'scope>,
    active: ActiveRegistrar<'scope>,
    state_scope: Option<Scope<'scope>>,
    cleanup_reporter: Option<CleanupReporter>,
}

impl<'scope> ViewOwnerToken<'scope> {
    fn new(parts: ViewOwnerTokenParts<'scope>) -> Self {
        Self {
            effect: parts.effect,
            previous_effect: parts.previous_effect,
            validate: parts.validate,
            cleanup: parts.cleanup,
            owned_scope: parts.owned_scope,
            completion: parts.completion,
            active: parts.active,
            state_scope: parts.state_scope,
            cleanup_reporter: parts.cleanup_reporter,
        }
    }

    pub fn effect_from(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.effect.call(inputs, callback, error_handler)
    }

    pub fn effect_with_previous<T, F>(
        &self,
        callback: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), callback, error_handler)
    }

    pub fn effect_with_previous_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        callback: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.previous_effect
            .register(inputs, callback, error_handler)
    }

    pub fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<OwnerState<'scope, T>> {
        if !self.is_active() {
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        Ok(match self.state_scope {
            Some(scope) => OwnerState::new_stored(scope.stored(Some(value))?, self.active.clone()),
            None => OwnerState::new(value, self.active.clone()),
        })
    }

    pub fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.validate.call(inputs)
    }

    pub fn on_cleanup(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.cleanup.call(cleanup, error_handler)
    }

    pub fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        self.owned_scope.call()
    }

    pub(crate) fn host_callback<F>(
        &self,
        callback: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<HostCallback>
    where
        F: FnMut(JsValue) -> SilexResult<()> + 'scope,
    {
        Ok(HostCallback {
            destination: HostDestination::Sender(self.completion.call_sender(Box::new(callback))?),
            gate: Rc::new(Cell::new(true)),
            error_handler: erase_error_handler(error_handler),
        })
    }

    pub(crate) fn host_callback_once<F>(
        &self,
        callback: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<HostCallback>
    where
        F: FnMut(JsValue) -> SilexResult<()> + 'scope,
    {
        Ok(HostCallback {
            destination: HostDestination::Once(self.completion.call_once(Box::new(callback))?),
            gate: Rc::new(Cell::new(true)),
            error_handler: erase_error_handler(error_handler),
        })
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.get()
    }

    pub(crate) fn cleanup_reporter(&self) -> Option<CleanupReporter> {
        self.cleanup_reporter.clone()
    }

    pub(crate) fn host_resource_for_callback<F>(
        &self,
        callback: &HostCallback,
        cancel: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<HostResourceHandle<'scope>>
    where
        F: FnOnce() + 'scope,
    {
        let callback_for_cancel = callback.clone();
        let resource = HostResourceHandle::with_gate(callback.gate.clone(), move || {
            callback_for_cancel.cancel();
            cancel();
        });
        self.register_host_resource(resource, error_handler)
    }

    pub(crate) fn host_resource_for_js_callback<F>(
        &self,
        callback: &HostCallback,
        resource: HostResourceHandle<'scope>,
        cancel: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<HostResourceHandle<'scope>>
    where
        F: FnOnce() + 'scope,
    {
        let callback_for_cancel = callback.clone();
        resource.install_cancel(move || {
            callback_for_cancel.cancel();
            cancel();
        });
        self.register_host_resource(resource, error_handler)
    }

    fn register_host_resource(
        &self,
        resource: HostResourceHandle<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<HostResourceHandle<'scope>> {
        if !self.is_active() {
            resource.cancel_once();
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        let owner_resource = resource.clone();
        if let Err(error) = self.on_cleanup(
            Box::new(move || {
                owner_resource.cancel_once();
                Ok(())
            }),
            error_handler,
        ) {
            resource.cancel_once();
            return Err(error);
        }
        Ok(resource)
    }
}

/// Mount-time capability shared by all view implementations.
pub trait ViewOwner<'scope> {
    fn effect_from(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>;
    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()>;
    fn on_cleanup(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>;
    fn token(&self) -> ViewOwnerToken<'scope>;
    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>>;
}

impl<'scope> ViewOwner<'scope> for ViewOwnerToken<'scope> {
    fn effect_from(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        ViewOwnerToken::effect_from(self, inputs, callback, error_handler)
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.validate_inputs(inputs)
    }

    fn on_cleanup(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        ViewOwnerToken::on_cleanup(self, cleanup, error_handler)
    }

    fn token(&self) -> ViewOwnerToken<'scope> {
        self.clone()
    }

    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        ViewOwnerToken::owned_scope(self)
    }
}

/// Adapter for a lexical child scope.
#[derive(Clone)]
pub struct ScopedViewOwner<'scope> {
    scope: Scope<'scope>,
    cleanup_reporter: Option<CleanupReporter>,
}

impl<'scope> ScopedViewOwner<'scope> {
    pub fn new(scope: Scope<'scope>) -> Self {
        Self {
            scope,
            cleanup_reporter: None,
        }
    }

    pub(crate) fn with_cleanup_reporter(
        scope: Scope<'scope>,
        cleanup_reporter: CleanupReporter,
    ) -> Self {
        Self {
            scope,
            cleanup_reporter: Some(cleanup_reporter),
        }
    }

    pub fn effect_with_previous_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        callback: F,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        T: 'scope,
        F: FnMut(Option<&T>) -> SilexResult<T> + 'scope,
    {
        self.scope
            .effect_with_previous_from(inputs, callback, error_handler)
            .map(|_| ())
    }

    pub fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<OwnerState<'scope, T>> {
        self.token().owner_state(value)
    }
}

impl<'scope> ViewOwner<'scope> for ScopedViewOwner<'scope> {
    fn effect_from(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope
            .effect_from(inputs, callback, error_handler)
            .map(|_| ())
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.scope.validate_inputs(inputs)
    }

    fn on_cleanup(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope.on_cleanup(cleanup, error_handler)
    }

    fn token(&self) -> ViewOwnerToken<'scope> {
        let scope_for_effect = self.scope;
        let scope_for_previous = self.scope;
        let scope_for_cleanup = self.scope;
        let scope_for_owned = self.scope;
        let scope_for_sender = self.scope;
        let scope_for_once = self.scope;
        let scope_for_active = self.scope;
        let scope_for_validate = self.scope;
        let cleanup_reporter = self.cleanup_reporter.clone();
        ViewOwnerToken::new(ViewOwnerTokenParts {
            effect: EffectRegistrar::new(move |inputs, callback, error_handler| {
                scope_for_effect
                    .effect_from(inputs, callback, error_handler)
                    .map(|_| ())
            }),
            previous_effect: PreviousEffectOwner::Scoped(scope_for_previous),
            validate: ValidationRegistrar::new(move |inputs| {
                scope_for_validate.validate_inputs(inputs)
            }),
            cleanup: CleanupRegistrar::new(move |cleanup, error_handler| {
                scope_for_cleanup.on_cleanup(cleanup, error_handler)
            }),
            owned_scope: OwnedScopeRegistrar::new(move || scope_for_owned.owned_scope()),
            completion: CompletionRegistrar::new(
                move |callback| scope_for_sender.completion_sender(unwind_safe(callback)),
                move |callback| scope_for_once.completion_once(unwind_safe(callback)),
            ),
            active: ActiveRegistrar::new(move || scope_for_active.is_active()),
            state_scope: Some(self.scope),
            cleanup_reporter,
        })
    }

    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        self.scope.owned_scope()
    }
}

pub(crate) struct OwnedViewOwner<'scope> {
    scope: Rc<OwnedScope<'scope>>,
    cleanup_reporter: Option<CleanupReporter>,
}

impl<'scope> OwnedViewOwner<'scope> {
    pub(crate) fn new(scope: Rc<OwnedScope<'scope>>) -> Self {
        Self {
            scope,
            cleanup_reporter: None,
        }
    }

    pub(crate) fn with_cleanup_reporter(
        scope: Rc<OwnedScope<'scope>>,
        cleanup_reporter: CleanupReporter,
    ) -> Self {
        Self {
            scope,
            cleanup_reporter: Some(cleanup_reporter),
        }
    }

    pub(crate) fn owner_state<T: 'scope>(&self, value: T) -> SilexResult<OwnerState<'scope, T>> {
        self.token().owner_state(value)
    }
}

impl<'scope> ViewOwner<'scope> for OwnedViewOwner<'scope> {
    fn effect_from(
        &self,
        inputs: RuntimeInputs,
        callback: ViewEffect<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope
            .effect_from(inputs, callback, error_handler)
            .map(|_| ())
    }

    fn validate_inputs(&self, inputs: &RuntimeInputs) -> SilexResult<()> {
        self.scope.validate_inputs(inputs)
    }

    fn on_cleanup(
        &self,
        cleanup: ViewCleanup<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.scope.on_cleanup(cleanup, error_handler)
    }

    fn token(&self) -> ViewOwnerToken<'scope> {
        let scope_for_effect = self.scope.clone();
        let scope_for_previous = self.scope.clone();
        let scope_for_cleanup = self.scope.clone();
        let scope_for_owned = self.scope.clone();
        let scope_for_sender = self.scope.clone();
        let scope_for_once = self.scope.clone();
        let scope_for_active = self.scope.clone();
        let scope_for_validate = self.scope.clone();
        let cleanup_reporter = self.cleanup_reporter.clone();
        ViewOwnerToken::new(ViewOwnerTokenParts {
            effect: EffectRegistrar::new(move |inputs, callback, error_handler| {
                scope_for_effect
                    .effect_from(inputs, callback, error_handler)
                    .map(|_| ())
            }),
            previous_effect: PreviousEffectOwner::Owned(scope_for_previous),
            validate: ValidationRegistrar::new(move |inputs| {
                scope_for_validate.validate_inputs(inputs)
            }),
            cleanup: CleanupRegistrar::new(move |cleanup, error_handler| {
                scope_for_cleanup.on_cleanup(cleanup, error_handler)
            }),
            owned_scope: OwnedScopeRegistrar::new(move || scope_for_owned.child()),
            completion: CompletionRegistrar::new(
                move |callback| scope_for_sender.completion_sender(unwind_safe(callback)),
                move |callback| scope_for_once.completion_once(unwind_safe(callback)),
            ),
            active: ActiveRegistrar::new(move || scope_for_active.is_active()),
            state_scope: None,
            cleanup_reporter,
        })
    }

    fn owned_scope(&self) -> SilexResult<OwnedScope<'scope>> {
        self.scope.child()
    }
}

/// Apply attributes to a view while preserving their scope boundary.
pub trait ApplyAttributes<'scope> {
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute<'scope>>) {}
}

/// Component prop wrapper used by generated builders.
pub enum Prop<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T: RxValue> Prop<'a, T> {
    pub fn new_borrowed(value: &'a T) -> Self {
        Self::Borrowed(value)
    }

    pub fn new_owned(value: T) -> Self {
        Self::Owned(value)
    }

    pub fn new(value: &'a T) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a, T: Clone> Prop<'a, T> {
    pub fn into_owned(self) -> T {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value.clone(),
        }
    }
}

impl<'a, T: Clone> Clone for Prop<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(value) => Self::Owned(value.clone()),
            Self::Borrowed(value) => Self::Owned((*value).clone()),
        }
    }
}

impl<'a, T: Copy> Copy for Prop<'a, T> {}

pub trait PropInto<T> {
    fn prop_into(self) -> T;
}

impl<T> PropInto<T> for T {
    fn prop_into(self) -> T {
        self
    }
}

impl<'a, T: Clone> PropInto<T> for Prop<'a, T> {
    fn prop_into(self) -> T {
        self.into_owned()
    }
}

impl<'scope, 'a, T> ApplyAttributes<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: ApplyAttributes<'scope>,
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        match self {
            Self::Owned(value) => value.apply_attributes(attrs),
            Self::Borrowed(value) => {
                let _ = (value, attrs);
            }
        }
    }
}

impl<'scope, 'a, T> View<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: View<'scope>,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        match self {
            Self::Owned(value) => value.mount(owner, parent, attrs, error_handler),
            Self::Borrowed(value) => value.mount(owner, parent, attrs, error_handler),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        match self {
            Self::Owned(value) => value.mount_owned(owner, parent, attrs, error_handler),
            Self::Borrowed(value) => value.mount(owner, parent, attrs, error_handler),
        }
    }
}

impl<'a, T: RxValue> RxValue for Prop<'a, T> {
    type Value = T::Value;
}

impl<'a, T> Prop<'a, T> {
    pub fn promote<'scope>(
        self,
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, T::Value>>
    where
        'a: 'scope,
        T: ReactiveSource<'scope> + Clone,
        T::Value: Sized + RxData + 'scope,
    {
        scope.promote(self.into_owned(), error_handler)
    }
}

impl<'a, T> Deref for Prop<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value,
        }
    }
}

impl<'a, T: Debug> Debug for Prop<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
    }
}

impl<'a, T: Display> Display for Prop<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
    }
}

macro_rules! impl_forward_binop_copy {
    ($trait:ident, $method:ident) => {
        impl<'a, T, Rhs> $trait<Rhs> for Prop<'a, T>
        where
            T: Copy + $trait<Rhs>,
        {
            type Output = <T as $trait<Rhs>>::Output;

            fn $method(self, rhs: Rhs) -> Self::Output {
                self.deref().$method(rhs)
            }
        }
    };
}

impl_forward_binop_copy!(Add, add);
impl_forward_binop_copy!(Sub, sub);
impl_forward_binop_copy!(Mul, mul);
impl_forward_binop_copy!(Div, div);

/// View conversion and mounting contract.
pub trait View<'scope> {
    fn into_any(self) -> AnyView<'scope>
    where
        Self: Sized + 'scope,
    {
        AnyView::new(self)
    }

    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>;

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized;
}

pub(crate) fn mount_composite<'scope, F>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: ViewErrorHandler<'scope>,
    mount: F,
) -> SilexResult<()>
where
    F: FnOnce(
        &dyn ViewOwner<'scope>,
        &Node,
        Vec<PendingAttribute<'scope>>,
        ViewErrorHandler<'scope>,
    ) -> SilexResult<()>,
{
    let scope = Rc::new(owner.owned_scope()?);
    let owner_token = owner.token();
    let provisional_owner = owner_token.cleanup_reporter().map_or_else(
        || OwnedViewOwner::new(scope.clone()),
        |reporter| OwnedViewOwner::with_cleanup_reporter(scope.clone(), reporter),
    );
    let fragment: Node = crate::document().create_document_fragment().into();

    if let Err(error) = mount(&provisional_owner, &fragment, attrs, error_handler) {
        return rollback_composite_scope_with_primary(owner, &scope, error);
    }

    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            let _ = scope_for_cleanup.dispose();
            Ok(())
        }),
        error_handler,
    ) {
        return rollback_composite_scope_with_primary(owner, &scope, error);
    }

    if let Err(error) = parent.append_child(&fragment).map_err(SilexError::fatal) {
        return rollback_composite_scope_with_primary(owner, &scope, error);
    }
    Ok(())
}

#[doc(hidden)]
pub fn mount_component<'scope, F>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: ViewErrorHandler<'scope>,
    mount: F,
) -> SilexResult<()>
where
    F: FnOnce(
        &dyn ViewOwner<'scope>,
        &Node,
        Vec<PendingAttribute<'scope>>,
        ViewErrorHandler<'scope>,
    ) -> SilexResult<()>,
{
    mount_composite(owner, parent, attrs, error_handler, mount)
}

fn rollback_composite_scope<'scope>(scope: &Rc<OwnedScope<'scope>>) -> Result<(), CleanupError> {
    match catch_unwind(AssertUnwindSafe(|| scope.dispose())) {
        Ok(result) => result,
        Err(panic) => resume_unwind(panic),
    }
}

fn rollback_composite_scope_with_primary<'scope>(
    owner: &dyn ViewOwner<'scope>,
    scope: &Rc<OwnedScope<'scope>>,
    primary: SilexError,
) -> SilexResult<()> {
    match rollback_composite_scope(scope) {
        Ok(()) => Err(primary),
        Err(cleanup) => {
            if let Some(reporter) = owner.token().cleanup_reporter() {
                reporter(cleanup);
            } else {
                let _ = cleanup.into_diagnostic();
            }
            Err(primary.into_fatal())
        }
    }
}

pub fn mount_text_node(parent: &Node, text: &str) -> SilexResult<()> {
    let document = crate::document();
    let node = document.create_text_node(text);
    parent.append_child(&node).map_err(SilexError::fatal)?;
    Ok(())
}

macro_rules! impl_text_view {
    ($ty:ty) => {
        impl<'scope> ApplyAttributes<'scope> for $ty {}

        impl<'scope> View<'scope> for $ty {
            fn mount(
                &self,
                owner: &dyn ViewOwner<'scope>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope>>,
                _error_handler: ViewErrorHandler<'scope>,
            ) -> SilexResult<()> {
                let _ = owner;
                mount_text_node(parent, self)
            }

            fn mount_owned(
                self,
                owner: &dyn ViewOwner<'scope>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope>>,
                _error_handler: ViewErrorHandler<'scope>,
            ) -> SilexResult<()>
            where
                Self: Sized,
            {
                let _ = owner;
                mount_text_node(parent, &self)
            }
        }
    };
}

impl_text_view!(String);

impl<'scope> ApplyAttributes<'scope> for &'scope str {}

impl<'scope> View<'scope> for &'scope str {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let _ = owner;
        mount_text_node(parent, self)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let _ = owner;
        mount_text_node(parent, self)
    }
}

impl<'scope> ApplyAttributes<'scope> for Cow<'scope, str> {}

impl<'scope> View<'scope> for Cow<'scope, str> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let _ = owner;
        mount_text_node(parent, self.as_ref())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let _ = owner;
        mount_text_node(parent, self.as_ref())
    }
}

macro_rules! impl_primitive_view {
    ($($ty:ty),*) => {
        $(
            impl<'scope> ApplyAttributes<'scope> for $ty {}

            impl<'scope> View<'scope> for $ty {
                fn mount(
                    &self,
                    owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
                    _error_handler: ViewErrorHandler<'scope>,
                ) -> SilexResult<()> {
                    let _ = owner;
                    mount_text_node(parent, &self.to_string())
                }

                fn mount_owned(
                    self,
                    owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
                    _error_handler: ViewErrorHandler<'scope>,
                ) -> SilexResult<()> where
                    Self: Sized,
                {
                    let _ = owner;
                    mount_text_node(parent, &self.to_string())
                }
            }
        )*
    };
}

impl_primitive_view!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64, bool, char
);

impl<'scope> ApplyAttributes<'scope> for () {}

impl<'scope> View<'scope> for () {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        Ok(())
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'scope, F, V> ApplyAttributes<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
}

impl<'scope, F, V> View<'scope> for F
where
    F: Fn() -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.clone()
            .mount_owned(owner, parent, attrs, error_handler)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_dynamic_view_universal(
            owner,
            parent,
            attrs,
            error_handler,
            RenderThunk::new(move |args| {
                let RenderArgs {
                    parent,
                    attrs,
                    owner: token,
                    error_handler,
                } = args;
                let view = self();
                view.mount_owned(&token, &parent, attrs, error_handler)
            }),
        )
    }
}

/// Shared dynamic-view mount kernel.
pub fn mount_dynamic_view_universal<'scope>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: ViewErrorHandler<'scope>,
    renderer: RenderThunk<'scope>,
) -> SilexResult<()> {
    mount_dynamic_view_universal_from(
        owner,
        parent,
        attrs,
        RuntimeInputs::new(),
        error_handler,
        renderer,
    )
}

pub(crate) fn mount_dynamic_view_universal_from<'scope>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: ViewErrorHandler<'scope>,
    renderer: RenderThunk<'scope>,
) -> SilexResult<()> {
    owner.validate_inputs(&inputs)?;
    let range = DomRange::append(parent, "dyn")?;
    let render = RowRender::new(move |args: RowRenderArgs<'scope, ()>| {
        let RowRenderArgs {
            parent,
            attrs,
            owner: token,
            error_handler,
            ..
        } = args;
        renderer.call(RenderArgs::new(parent, attrs, token, error_handler))
    });
    let token = owner.token();
    let row = RowController::new(
        &token,
        RowControllerConfig {
            range,
            render,
            render_inputs: inputs,
            attrs,
            item: (),
            index: 0,
            stateful: false,
            error_handler,
        },
    )?;
    let row_state = owner.token().owner_state(Some(row))?;
    let cleanup_state = row_state.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            if let Some(mut row) = cleanup_state.take_for_cleanup().flatten() {
                row.dispose();
            }
            Ok(())
        }),
        error_handler,
    ) {
        if let Some(mut row) = row_state.take_for_cleanup().flatten() {
            row.dispose();
        }
        return Err(error);
    }
    Ok(())
}

/// Dynamic view mount with a persistent row owner keyed by the current key.
pub fn mount_dynamic_view_cached<'scope, K, KeyFn, RenderFn>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: ViewErrorHandler<'scope>,
    key_fn: KeyFn,
    renderer: RenderFn,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
    RenderFn: Fn(K, (Node, Vec<PendingAttribute<'scope>>)) -> SilexResult<()> + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, K>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            ..
        } = args;
        renderer(key, (parent, attrs))
    });
    mount_keyed_dynamic_view(KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key: true,
    })
}

pub fn mount_branch_cached<'scope, K, KeyFn, BranchFn>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: ViewErrorHandler<'scope>,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
    BranchFn: Fn(K) -> AnyView<'scope> + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, K>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            owner: token,
            error_handler,
            ..
        } = args;
        branch_fn(key).mount_owned(&token, &parent, attrs, error_handler)
    });
    mount_keyed_dynamic_view(KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key: true,
    })
}

/// The identity and render snapshot produced by one stable branch evaluation.
///
/// Stable branches compare only `key`; the snapshot is delivered to the branch
/// renderer when a new row is mounted.
#[derive(Clone)]
pub struct BranchEvaluation<K, S> {
    key: K,
    snapshot: S,
}

impl<K, S> BranchEvaluation<K, S> {
    pub fn new(key: K, snapshot: S) -> Self {
        Self { key, snapshot }
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_parts(self) -> (K, S) {
        (self.key, self.snapshot)
    }
}

impl<K: PartialEq, S> PartialEq for BranchEvaluation<K, S> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<K: Eq, S> Eq for BranchEvaluation<K, S> {}

/// Mount a stable branch whose evaluation can report a runtime error.
pub fn mount_branch_stable_cached<'scope, K, S, KeyFn, BranchFn>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: ViewErrorHandler<'scope>,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    S: Clone + 'scope,
    KeyFn: Fn() -> SilexResult<BranchEvaluation<K, S>> + Clone + 'scope,
    BranchFn: Fn(BranchEvaluation<K, S>) -> AnyView<'scope> + 'scope,
{
    let render = RowRender::new(move |args: RowRenderArgs<'scope, BranchEvaluation<K, S>>| {
        let RowRenderArgs {
            item: key,
            parent,
            attrs,
            owner: token,
            error_handler,
            ..
        } = args;
        branch_fn(key).mount_owned(&token, &parent, attrs, error_handler)
    });
    mount_keyed_dynamic_view_result(KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key: false,
    })
}

struct BranchState<'scope, K> {
    range: DomRange,
    row: Option<RowController<'scope, K>>,
    key: Option<K>,
    render: RowRender<'scope, K>,
    attrs: Vec<PendingAttribute<'scope>>,
}

struct KeyedDynamicMountArgs<'owner, 'scope, K, KeyFn> {
    owner: &'owner dyn ViewOwner<'scope>,
    parent: &'owner Node,
    attrs: Vec<PendingAttribute<'scope>>,
    inputs: RuntimeInputs,
    error_handler: ViewErrorHandler<'scope>,
    key_fn: KeyFn,
    render: RowRender<'scope, K>,
    update_same_key: bool,
}

fn mount_keyed_dynamic_view<'owner, 'scope, K, KeyFn>(
    args: KeyedDynamicMountArgs<'owner, 'scope, K, KeyFn>,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> K + Clone + 'scope,
{
    let KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key,
    } = args;
    mount_keyed_dynamic_view_result(KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn: move || Ok(key_fn()),
        render,
        update_same_key,
    })
}

fn mount_keyed_dynamic_view_result<'scope, K, KeyFn>(
    args: KeyedDynamicMountArgs<'_, 'scope, K, KeyFn>,
) -> SilexResult<()>
where
    K: PartialEq + Clone + 'scope,
    KeyFn: Fn() -> SilexResult<K> + Clone + 'scope,
{
    let KeyedDynamicMountArgs {
        owner,
        parent,
        attrs,
        inputs,
        error_handler,
        key_fn,
        render,
        update_same_key,
    } = args;
    owner.validate_inputs(&inputs)?;
    let scope = Rc::new(owner.owned_scope()?);
    let local_owner = OwnedViewOwner::new(scope.clone());
    let range = DomRange::append(parent, "branch")?;
    let state = local_owner.token().owner_state(BranchState {
        range,
        row: None,
        key: None,
        render,
        attrs,
    })?;
    let cleanup_state = state.clone();
    let cleanup_range = state.with(|state| state.range.clone())?;
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            let Some(mut state) = cleanup_state.take_for_cleanup() else {
                return Ok(());
            };
            state.key = None;
            let row = state.row.take();
            let range = state.range.clone();
            let panic = row
                .map(|mut row| catch_unwind(AssertUnwindSafe(move || row.dispose())))
                .and_then(Result::err);
            range.remove();
            if let Some(panic) = panic {
                resume_unwind(panic);
            }
            Ok(())
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        cleanup_range.remove();
        return Err(error);
    }

    let token = local_owner.token();
    let effect_state = state.clone();
    if let Err(error) = local_owner.effect_from(
        inputs,
        Box::new(move || -> SilexResult<()> {
            let mut state = effect_state.take()?;
            let result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
                let key = key_fn()?;
                let same_key = state.key.as_ref().is_some_and(|current| current == &key);
                if same_key {
                    if update_same_key {
                        state
                            .row
                            .as_mut()
                            .ok_or_else(|| {
                                SilexError::fatal(SilexErrorKind::Framework(
                                    "dynamic row is missing for current key".to_string(),
                                ))
                            })?
                            .update(key, 0)?;
                    } else if state.row.is_none() {
                        return Err(SilexError::fatal(SilexErrorKind::Framework(
                            "dynamic row is missing for current key".to_string(),
                        )));
                    }
                    return Ok(());
                }

                let (outer_range, render, attrs, old_row, old_key) = {
                    (
                        state.range.clone(),
                        state.render.clone(),
                        state.attrs.clone(),
                        state.row.take(),
                        state.key.take(),
                    )
                };
                let row_range = match DomRange::before(&outer_range.end, "branch-row") {
                    Ok(row_range) => row_range,
                    Err(error) => {
                        state.row = old_row;
                        state.key = old_key;
                        return Err(error);
                    }
                };
                let row = match RowController::new(
                    &token,
                    RowControllerConfig {
                        range: row_range,
                        render,
                        render_inputs: RuntimeInputs::new(),
                        attrs,
                        item: key.clone(),
                        index: 0,
                        stateful: false,
                        error_handler,
                    },
                ) {
                    Ok(row) => row,
                    Err(error) => {
                        state.row = old_row;
                        state.key = old_key;
                        return Err(error);
                    }
                };

                let old_panic = old_row
                    .map(|mut row| catch_unwind(AssertUnwindSafe(move || row.dispose())))
                    .and_then(Result::err);
                state.key = Some(key);
                state.row = Some(row);
                if let Some(panic) = old_panic {
                    resume_unwind(panic);
                }
                Ok(())
            }));
            effect_state.replace(state)?;
            match result {
                Ok(result) => result,
                Err(panic) => {
                    let message = if let Some(value) = panic.downcast_ref::<&str>() {
                        format!("Panic in Dynamic Branch: {value}")
                    } else if let Some(value) = panic.downcast_ref::<String>() {
                        format!("Panic in Dynamic Branch: {value}")
                    } else {
                        "Panic in Dynamic Branch: unknown panic".to_string()
                    };
                    Err(SilexError::fatal(SilexErrorKind::Javascript(message)))
                }
            }
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            let _ = scope_for_cleanup.dispose();
            Ok(())
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    Ok(())
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Option<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        if let Some(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Option<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        if let Some(value) = self {
            value.mount(owner, parent, attrs, error_handler)
        } else {
            Ok(())
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        if let Some(value) = self {
            value.mount_owned(owner, parent, attrs, error_handler)
        } else {
            Ok(())
        }
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Vec<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Vec<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.iter().enumerate() {
                    value.mount(
                        transaction_owner,
                        fragment,
                        if index == 0 {
                            attrs.clone()
                        } else {
                            Vec::new()
                        },
                        error_handler,
                    )?;
                }
                Ok(())
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.into_iter().enumerate() {
                    value.mount_owned(
                        transaction_owner,
                        fragment,
                        if index == 0 {
                            attrs.clone()
                        } else {
                            Vec::new()
                        },
                        error_handler,
                    )?;
                }
                Ok(())
            },
        )
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>, const N: usize> ApplyAttributes<'scope>
    for [V; N]
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, V: View<'scope>, const N: usize> View<'scope> for [V; N] {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.iter().enumerate() {
                    value.mount(
                        transaction_owner,
                        fragment,
                        if index == 0 {
                            attrs.clone()
                        } else {
                            Vec::new()
                        },
                        error_handler,
                    )?;
                }
                Ok(())
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.into_iter().enumerate() {
                    value.mount_owned(
                        transaction_owner,
                        fragment,
                        if index == 0 {
                            attrs.clone()
                        } else {
                            Vec::new()
                        },
                        error_handler,
                    )?;
                }
                Ok(())
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewNil;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewCons<H, T>(pub H, pub T);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropMissing;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropFixed;

impl<'scope> ApplyAttributes<'scope> for ViewNil {}

impl<'scope> View<'scope> for ViewNil {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        Ok(())
    }

    fn mount_owned(
        self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'scope, H: ApplyAttributes<'scope>, T: ApplyAttributes<'scope>> ApplyAttributes<'scope>
    for ViewCons<H, T>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

impl<'scope, H: View<'scope>, T: View<'scope>> View<'scope> for ViewCons<H, T> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                self.0
                    .mount(transaction_owner, fragment, attrs, error_handler)?;
                self.1
                    .mount(transaction_owner, fragment, Vec::new(), error_handler)
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let ViewCons(head, tail) = self;
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                head.mount_owned(transaction_owner, fragment, attrs, error_handler)?;
                tail.mount_owned(transaction_owner, fragment, Vec::new(), error_handler)
            },
        )
    }
}

#[macro_export]
macro_rules! chain {
    () => {
        $crate::view::ViewNil
    };
    ($head:expr $(,)?) => {
        $crate::view::ViewCons($head, $crate::view::ViewNil)
    };
    ($head:expr, $($tail:expr),+ $(,)?) => {
        $crate::view::ViewCons($head, $crate::chain!($($tail),+))
    };
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for SilexResult<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        if let Ok(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for SilexResult<V> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        match self {
            Ok(value) => value.mount(owner, parent, attrs, error_handler),
            Err(error) => Err(error.clone()),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        match self {
            Ok(value) => value.mount_owned(owner, parent, attrs, error_handler),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostResourceHandle, ScopedViewOwner, ViewOwner};
    use silex_core::{Runtime, SilexError, SilexErrorKind};
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    use wasm_bindgen::JsValue;

    #[test]
    fn host_callback_is_gated_after_root_dispose() {
        let seen = Rc::new(Cell::new(0));
        let seen_in_callback = seen.clone();
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        let bridge = {
            let scope = root.scope();
            let owner = ScopedViewOwner::new(scope);
            let token = owner.token();
            let handler = scope
                .error_handler(|_| {})
                .expect("error handler should register");
            let bridge = token
                .host_callback(
                    move |_| {
                        seen_in_callback.set(seen_in_callback.get() + 1);
                        Ok(())
                    },
                    handler,
                )
                .expect("host callback should register");
            assert!(bridge.dispatch(JsValue::UNDEFINED));
            bridge
        };

        assert_eq!(seen.get(), 1);
        root.dispose().expect("root cleanup should succeed");
        assert!(!bridge.dispatch(JsValue::UNDEFINED));
        assert_eq!(seen.get(), 1);
    }

    #[test]
    fn host_callback_reports_completion_errors_after_callback_returns() {
        let handled = Rc::new(Cell::new(0));
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        let bridge = {
            let scope = root.scope();
            let (_, set_signal) = scope.signal(0).expect("signal should initialize");
            let handled_in_handler = handled.clone();
            let handler = scope
                .error_handler(move |error| {
                    assert!(matches!(
                        error,
                        SilexError::Fatal(SilexErrorKind::Framework(message)) if message == "host"
                    ));
                    handled_in_handler.set(handled_in_handler.get() + 1);
                    set_signal.set(1).expect("signal should be writable");
                })
                .expect("error handler should register");
            let owner = ScopedViewOwner::new(scope);
            owner
                .token()
                .host_callback(
                    |_| {
                        Err(SilexError::fatal(SilexErrorKind::Framework(String::from(
                            "host",
                        ))))
                    },
                    handler,
                )
                .expect("host callback should register")
        };

        assert!(bridge.dispatch(JsValue::UNDEFINED));
        assert!(bridge.dispatch(JsValue::UNDEFINED));
        assert_eq!(handled.get(), 2);
    }

    #[test]
    fn host_callback_handler_panic_closes_the_destination() {
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        let bridge = {
            let scope = root.scope();
            let handler = scope
                .error_handler(|_| panic!("host error handler panic"))
                .expect("error handler should register");
            let owner = ScopedViewOwner::new(scope);
            owner
                .token()
                .host_callback(
                    |_| {
                        Err(SilexError::fatal(SilexErrorKind::Framework(String::from(
                            "host",
                        ))))
                    },
                    handler,
                )
                .expect("host callback should register")
        };

        let result = catch_unwind(AssertUnwindSafe(|| bridge.dispatch(JsValue::UNDEFINED)));
        assert!(result.is_err());
        assert!(!bridge.dispatch(JsValue::UNDEFINED));
        root.dispose().expect("root cleanup should succeed");
    }

    #[test]
    fn host_resource_cancellation_is_idempotent() {
        let cancelled = Rc::new(Cell::new(0));
        let cancelled_in_cleanup = cancelled.clone();
        let handle = HostResourceHandle::with_gate(Rc::new(Cell::new(true)), move || {
            cancelled_in_cleanup.set(cancelled_in_cleanup.get() + 1);
        });
        let clone = handle.clone();

        handle.cancel();
        clone.cancel();
        drop(clone);
        drop(handle);

        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn owner_keeps_resource_alive_when_returned_handle_is_dropped() {
        let cancelled = Rc::new(Cell::new(0));
        let cancelled_in_cleanup = cancelled.clone();
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");
        {
            let scope = root.scope();
            let owner = ScopedViewOwner::new(scope);
            let token = owner.token();
            let handler = scope
                .error_handler(|_| {})
                .expect("error handler should register");
            let callback = token
                .host_callback(|_| Ok(()), handler)
                .expect("host callback should register");
            let handle = token
                .host_resource_for_callback(
                    &callback,
                    move || {
                        cancelled_in_cleanup.set(cancelled_in_cleanup.get() + 1);
                    },
                    handler,
                )
                .expect("host resource should register");
            drop(handle);
        }
        assert_eq!(cancelled.get(), 0);
        root.dispose().expect("root cleanup should succeed");
        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn owner_rejects_foreign_inputs_before_effect_registration() {
        let mut first = Runtime::new();
        let mut second = Runtime::new();
        let inputs = first
            .child(|scope| {
                let (source, _) = scope.signal(1i32).expect("signal should initialize");
                scope
                    .promote(
                        source,
                        scope
                            .error_handler(|_| {})
                            .expect("error handler should register"),
                    )
                    .expect("promotion should initialize")
                    .runtime_inputs()
            })
            .expect("child scope should initialize");
        let runs = Rc::new(Cell::new(0));
        let runs_for_effect = runs.clone();

        second
            .child(|scope| {
                let errors = Rc::new(RefCell::new(Vec::new()));
                let errors_for_reporter = errors.clone();
                let handler = scope
                    .error_handler(move |error| errors_for_reporter.borrow_mut().push(error))
                    .expect("error handler should register");
                let owner = ScopedViewOwner::new(scope);
                assert!(owner.validate_inputs(&inputs).is_err());
                assert!(
                    owner
                        .effect_from(
                            inputs,
                            Box::new(move || {
                                runs_for_effect.set(runs_for_effect.get() + 1);
                                Ok(())
                            }),
                            handler,
                        )
                        .is_err()
                );
                assert!(errors.borrow().is_empty());
            })
            .expect("child scope should initialize");

        assert_eq!(runs.get(), 0);
    }

    #[test]
    fn explicit_handlers_route_errors_locally() {
        let outer_errors = Rc::new(RefCell::new(Vec::<String>::new()));
        let inner_errors = Rc::new(RefCell::new(Vec::<String>::new()));
        let outer_errors_for_reporter = outer_errors.clone();
        let inner_errors_for_reporter = inner_errors.clone();
        let mut runtime = Runtime::new();

        runtime
            .child(|scope| {
                let outer_handler = scope
                    .error_handler(move |error| {
                        outer_errors_for_reporter
                            .borrow_mut()
                            .push(error.to_string());
                    })
                    .expect("error handler should register");
                let inner_handler = scope
                    .error_handler(move |error| {
                        inner_errors_for_reporter
                            .borrow_mut()
                            .push(error.to_string());
                    })
                    .expect("error handler should register");
                outer_handler
                    .handle(SilexError::fatal(SilexErrorKind::Framework(
                        "outer".to_string(),
                    )))
                    .expect("outer handler should be active");
                inner_handler
                    .handle(SilexError::fatal(SilexErrorKind::Framework(
                        "inner".to_string(),
                    )))
                    .expect("inner handler should be active");
                outer_handler
                    .handle(SilexError::fatal(SilexErrorKind::Framework(
                        "outer-again".to_string(),
                    )))
                    .expect("outer handler should be active");
            })
            .expect("child scope should initialize");

        assert_eq!(
            outer_errors.borrow().as_slice(),
            [
                "Fatal: Framework Error: outer",
                "Fatal: Framework Error: outer-again"
            ]
        );
        assert_eq!(
            inner_errors.borrow().as_slice(),
            ["Fatal: Framework Error: inner"]
        );
    }

    #[test]
    fn owner_state_rejects_late_access_but_cleanup_can_take_value() {
        let late_access_rejected = Rc::new(Cell::new(false));
        let cleanup_value = Rc::new(Cell::new(0));
        let late_access_rejected_for_cleanup = late_access_rejected.clone();
        let cleanup_value_for_cleanup = cleanup_value.clone();
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("root should start");

        {
            let scope = root.scope();
            let owner = ScopedViewOwner::new(scope);
            let token = owner.token();
            let state = token.owner_state(41).expect("owner state should be active");
            assert_eq!(
                state
                    .with(|value| *value)
                    .expect("state should be readable"),
                41
            );

            owner
                .on_cleanup(
                    Box::new(move || {
                        late_access_rejected_for_cleanup.set(state.with(|_| ()).is_err());
                        if let Some(value) = state.take_for_cleanup() {
                            cleanup_value_for_cleanup.set(value);
                        }
                        Ok(())
                    }),
                    scope
                        .error_handler(|_| {})
                        .expect("error handler should register"),
                )
                .expect("cleanup should register");
        }

        root.dispose().expect("root cleanup should succeed");
        assert!(late_access_rejected.get());
        assert_eq!(cleanup_value.get(), 41);
    }
}
