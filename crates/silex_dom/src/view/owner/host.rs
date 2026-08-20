use super::state::{ResourceGate, SharedCell};
use silex_core::{
    CallbackInvokeError, CloseError, ClosePhase, CloseSource, CompletionOnce, CompletionSender,
    CompletionSubmitError, ReactiveError, SilexError, SilexErrorKind,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue};

type CancelResult = Result<(), CloseError>;
type CancelAction<'scope> = SharedCell<Option<Box<dyn FnOnce() -> CancelResult + 'scope>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourceState {
    Active,
    Cancelling,
    Cancelled,
}

#[derive(Clone)]
pub(crate) struct JsCallbackResource {
    callback: SharedCell<Option<JsValue>>,
    active: ResourceGate,
}

impl JsCallbackResource {
    fn new(active: ResourceGate, callback: Option<JsValue>) -> Self {
        Self {
            callback: SharedCell::new(callback),
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

struct HostResourceInner<'scope> {
    cancel: CancelAction<'scope>,
    active: ResourceGate,
    state: Rc<Cell<ResourceState>>,
    callback: Option<JsCallbackResource>,
}

#[derive(Clone)]
pub(super) struct HostResourceLease<'scope>(Rc<HostResourceInner<'scope>>);

pub struct HostResource<'scope> {
    inner: Rc<HostResourceInner<'scope>>,
}

impl<'scope> HostResourceInner<'scope> {
    fn cancel_once(&self) -> CancelResult {
        if self.state.replace(ResourceState::Cancelling) != ResourceState::Active {
            debug_assert!(matches!(
                self.state.get(),
                ResourceState::Cancelling | ResourceState::Cancelled
            ));
            return Ok(());
        }
        self.active.set(false);
        if let Some(callback) = &self.callback {
            callback.cancel_once();
        }
        let cancel = self.cancel.take();
        let result = cancel.map(|cancel| catch_unwind(AssertUnwindSafe(cancel)));
        self.state.set(ResourceState::Cancelled);
        match result {
            None | Some(Ok(Ok(()))) => Ok(()),
            Some(Ok(Err(error))) => Err(error),
            Some(Err(panic)) => Err(CloseError::from_panic(panic)),
        }
    }

    fn finish(&self) {
        if self.state.replace(ResourceState::Cancelled) != ResourceState::Active {
            return;
        }
        self.active.set(false);
        if let Some(callback) = &self.callback {
            callback.cancel_once();
        }
        let _ = self.cancel.take();
    }

    fn is_active(&self) -> bool {
        self.state.get() == ResourceState::Active && self.active.get()
    }
}

impl HostResourceLease<'_> {
    pub(super) fn cancel_once(&self) -> CancelResult {
        self.0.cancel_once()
    }
}

impl<'scope> HostResource<'scope> {
    pub(crate) fn inactive() -> Self {
        Self::with_parts(Rc::new(Cell::new(false)), None, None)
    }

    pub(super) fn with_gate<F>(active: ResourceGate, cancel: F) -> Self
    where
        F: FnOnce() -> CancelResult + 'scope,
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
        cancel: Option<Box<dyn FnOnce() -> CancelResult + 'scope>>,
        callback: Option<JsCallbackResource>,
    ) -> Self {
        let state = if active.get() {
            ResourceState::Active
        } else {
            ResourceState::Cancelled
        };
        Self {
            inner: Rc::new(HostResourceInner {
                cancel: SharedCell::new(cancel),
                active,
                state: Rc::new(Cell::new(state)),
                callback,
            }),
        }
    }

    pub(crate) fn set_js_callback(&self, callback: JsValue) {
        self.callback_resource().set_callback(callback);
    }

    pub(crate) fn js_callback_function(&self) -> js_sys::Function {
        self.callback_resource().callback_function()
    }

    pub(crate) fn callback_resource(&self) -> JsCallbackResource {
        self.inner
            .callback
            .clone()
            .expect("host callback resource is present")
    }

    pub(super) fn owner_lease(&self) -> HostResourceLease<'scope> {
        HostResourceLease(self.inner.clone())
    }

    pub(super) fn install_cancel<F>(&self, cancel: F)
    where
        F: FnOnce() -> CancelResult + 'scope,
    {
        self.inner.cancel.with_mut(|current| {
            debug_assert!(current.is_none());
            *current = Some(Box::new(cancel));
        });
    }

    pub(crate) fn cancel_once(&self) -> CancelResult {
        self.inner.cancel_once()
    }

    /// Cancel the host resource. Repeated calls are harmless.
    pub fn cancel(&self) -> Result<(), SilexError> {
        self.cancel_once()
            .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
    }

    /// Mark a one-shot host resource as completed without running its physical
    /// cancellation action.
    pub fn finish(&self) {
        self.inner.finish();
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_active()
    }
}

#[derive(Clone)]
pub(super) enum HostDestination {
    Once(CompletionOnce<JsValue>),
    Sender(CompletionSender<JsValue>),
}

impl HostDestination {
    fn dispatch(&self, payload: JsValue) -> Result<bool, CompletionSubmitError<SilexError>> {
        match self {
            Self::Once(destination) => destination.submit(payload),
            Self::Sender(destination) => destination.submit(payload),
        }
    }

    fn cancel(&self) -> Result<(), CloseError> {
        match self {
            Self::Once(destination) => destination.cancel(),
            Self::Sender(destination) => destination.cancel(),
        }
    }
}

/// A `'static` browser closure's only path back into a scoped view.
#[derive(Clone)]
pub(crate) struct HostCallback {
    pub(super) destination: HostDestination,
    pub(super) gate: ResourceGate,
    pub(super) error_completion: CompletionSender<SilexError>,
    pub(super) state: Rc<Cell<CallbackState>>,
    pub(super) close_failures: SharedCell<Vec<CloseError>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CallbackState {
    Active,
    ReportingError,
    Cancelling,
    Closed,
}

// HostCallback only carries framework-owned gate and Completion state. User
// callbacks are invoked behind Completion's panic cleanup boundary.
impl UnwindSafe for HostCallback {}

impl HostCallback {
    fn record_close_failure(&self, error: CloseError, source: CloseSource) {
        self.close_failures.with_mut(|failures| {
            failures.push(error.with_context(ClosePhase::Runtime, source));
        });
    }

    fn take_close_error(&self) -> Option<CloseError> {
        CloseError::combine(self.close_failures.take())
    }

    fn report_error(&self, error: SilexError, source: CloseSource) {
        if self.state.replace(CallbackState::ReportingError) != CallbackState::Active {
            return;
        }
        let result = catch_unwind(AssertUnwindSafe(|| self.error_completion.submit(error)));
        match result {
            Ok(Ok(_)) => {
                if self.state.get() == CallbackState::ReportingError {
                    self.state.set(CallbackState::Active);
                }
            }
            Ok(Err(error)) => {
                self.record_close_failure(
                    CloseError::from_panic(Box::new(format!(
                        "host callback error report failed for {source:?}: {error:?}"
                    ))),
                    CloseSource::Handler,
                );
                self.gate.set(false);
                self.state.set(CallbackState::Closed);
            }
            Err(panic) => {
                self.record_close_failure(
                    CloseError::from_panic(Box::new("host callback error handler panicked")),
                    CloseSource::Handler,
                );
                self.gate.set(false);
                self.state.set(CallbackState::Closed);
                resume_unwind(panic);
            }
        }
    }

    pub(crate) fn dispatch(&self, payload: JsValue) -> bool {
        if !self.gate.get() || self.state.get() != CallbackState::Active {
            return false;
        }
        match self.destination.dispatch(payload) {
            Ok(active) => active,
            Err(CompletionSubmitError::Callback(CallbackInvokeError::User(error))) => {
                self.report_error(error, CloseSource::UserCallback);
                self.gate.get() && self.state.get() == CallbackState::Active
            }
            Err(CompletionSubmitError::Callback(CallbackInvokeError::Runtime(error))) => {
                self.report_error(SilexError::fatal(error), CloseSource::Dispose);
                self.gate.get() && self.state.get() == CallbackState::Active
            }
            Err(CompletionSubmitError::Callback(CallbackInvokeError::Handler(error))) => {
                self.report_error(
                    SilexError::fatal(ReactiveError::Handler(error)),
                    CloseSource::Handler,
                );
                self.gate.get() && self.state.get() == CallbackState::Active
            }
            Err(CompletionSubmitError::Close(error)) => {
                self.record_close_failure(*error, CloseSource::Destination);
                self.gate.set(false);
                self.state.set(CallbackState::Closed);
                false
            }
            Err(CompletionSubmitError::CallbackAndClose { callback, close }) => {
                let callback = match callback {
                    CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
                    CallbackInvokeError::User(error) => error,
                    CallbackInvokeError::Handler(error) => {
                        SilexError::fatal(ReactiveError::Handler(error))
                    }
                };
                self.report_error(callback, CloseSource::UserCallback);
                self.record_close_failure(*close, CloseSource::Destination);
                self.gate.get() && self.state.get() == CallbackState::Active
            }
        }
    }

    pub(crate) fn finish(&self) {
        self.gate.set(false);
        self.state.set(CallbackState::Closed);
    }

    pub(crate) fn cancel(&self) -> Result<(), CloseError> {
        if self.state.replace(CallbackState::Cancelling) == CallbackState::Closed {
            return self.take_close_error().map_or(Ok(()), Err);
        }
        self.gate.set(false);
        let result = catch_unwind(AssertUnwindSafe(|| self.destination.cancel()));
        self.state.set(CallbackState::Closed);
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => self.record_close_failure(error, CloseSource::Destination),
            Err(panic) => {
                self.record_close_failure(CloseError::from_panic(panic), CloseSource::Destination)
            }
        }
        self.take_close_error().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::HostResource;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn host_resource_cancel_is_physical_once() {
        let cancel_count = Rc::new(Cell::new(0));
        let cancel_count_for_action = cancel_count.clone();
        let resource = HostResource::with_gate(Rc::new(Cell::new(true)), move || {
            cancel_count_for_action.set(cancel_count_for_action.get() + 1);
            Ok(())
        });

        assert!(resource.is_active());
        let _ = resource.cancel();
        let _ = resource.cancel();

        assert!(!resource.is_active());
        assert_eq!(cancel_count.get(), 1);
    }

    #[test]
    fn host_resource_is_cancelled_after_physical_cancel_panics() {
        let resource = HostResource::with_gate(Rc::new(Cell::new(true)), || {
            panic!("physical cancellation failure");
        });

        assert!(resource.cancel().is_err());
        assert!(!resource.is_active());
        let _ = resource.cancel();
        assert!(!resource.is_active());
    }
}
