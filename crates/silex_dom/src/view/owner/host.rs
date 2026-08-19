use super::state::{ResourceGate, SharedCell};
use silex_core::{
    CallbackInvokeError, CloseError, CompletionOnce, CompletionSender, CompletionSubmitError,
    ReactiveError, SilexError, SilexErrorKind,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue};

type CancelAction<'scope> = SharedCell<Option<Box<dyn FnOnce() + 'scope>>>;

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

    pub(super) fn with_gate<F>(active: ResourceGate, cancel: F) -> Self
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
            cancel: SharedCell::new(cancel),
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

    pub(super) fn install_cancel<F>(&self, cancel: F)
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
}

// HostCallback only carries framework-owned gate and Completion state. User
// callbacks are invoked behind Completion's panic cleanup boundary.
impl UnwindSafe for HostCallback {}

impl HostCallback {
    fn report_error(&self, error: SilexError) {
        let result = catch_unwind(AssertUnwindSafe(|| self.error_completion.submit(error)));
        if let Ok(Err(_)) | Err(_) = result {
            let _ = catch_unwind(AssertUnwindSafe(|| self.cancel()));
            if let Err(panic) = result {
                resume_unwind(panic);
            }
        }
    }

    pub(crate) fn dispatch(&self, payload: JsValue) -> bool {
        if !self.gate.get() {
            return false;
        }
        match self.destination.dispatch(payload) {
            Ok(active) => active,
            Err(CompletionSubmitError::Callback(CallbackInvokeError::User(error))) => {
                self.report_error(error);
                self.gate.get()
            }
            Err(CompletionSubmitError::Callback(CallbackInvokeError::Runtime(error))) => {
                self.report_error(SilexError::fatal(error));
                self.gate.get()
            }
            Err(CompletionSubmitError::Callback(CallbackInvokeError::Handler(error))) => {
                self.report_error(SilexError::fatal(ReactiveError::Handler(error)));
                self.gate.get()
            }
            Err(CompletionSubmitError::Close(error)) => {
                self.report_error(SilexError::fatal(SilexErrorKind::Close(*error)));
                self.gate.get()
            }
            Err(CompletionSubmitError::CallbackAndClose { callback, close }) => {
                let callback = match callback {
                    CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
                    CallbackInvokeError::User(error) => error,
                    CallbackInvokeError::Handler(error) => {
                        SilexError::fatal(ReactiveError::Handler(error))
                    }
                };
                self.report_error(callback);
                self.report_error(SilexError::fatal(SilexErrorKind::Close(*close)));
                self.gate.get()
            }
        }
    }

    pub(crate) fn finish(&self) {
        self.gate.set(false);
    }

    pub(crate) fn cancel(&self) {
        self.gate.set(false);
        if let Err(error) = self.destination.cancel() {
            self.report_error(SilexError::fatal(SilexErrorKind::Close(error)));
        }
    }
}
