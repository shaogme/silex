use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use wasm_bindgen::JsValue;

use crate::Scope;

#[derive(Debug, Clone)] // Clone to allow easy propagation in closures if needed
pub enum SilexError {
    Dom(String),
    Reactivity(String),
    Javascript(String),
}

#[derive(Clone)]
pub struct ErrorContext(Rc<dyn Fn(SilexError)>);

impl fmt::Display for SilexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SilexError::Dom(msg) => write!(f, "DOM Error: {}", msg),
            SilexError::Reactivity(msg) => write!(f, "Reactivity Error: {}", msg),
            SilexError::Javascript(msg) => write!(f, "JavaScript Error: {}", msg),
        }
    }
}

impl std::error::Error for SilexError {}

impl From<JsValue> for SilexError {
    fn from(value: JsValue) -> Self {
        let msg = value.as_string().unwrap_or_else(|| format!("{:?}", value));
        SilexError::Javascript(msg)
    }
}

pub type SilexResult<T> = Result<T, SilexError>;

thread_local! {
    static ERROR_CONTEXT_STACK: RefCell<Vec<ErrorContext>> = const { RefCell::new(Vec::new()) };
}

impl ErrorContext {
    /// Creates a new `ErrorContext` with the given error handler callback.
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(SilexError) + 'static,
    {
        Self(Rc::new(handler))
    }

    /// Triggers the error handler held by this `ErrorContext`.
    pub fn handle(&self, err: SilexError) {
        (self.0)(err);
    }

    /// Pushes `self` onto the thread-local error context stack and associates
    /// its removal with an explicit reactive scope.
    pub fn push<'scope, 'run>(self, scope: &Scope<'scope, 'run>) {
        ERROR_CONTEXT_STACK.with(|stack| {
            stack.borrow_mut().push(self);
        });
        scope.on_cleanup(move || {
            Self::pop();
        });
    }

    /// Pops the top `ErrorContext` from the thread-local stack.
    pub fn pop() -> Option<ErrorContext> {
        ERROR_CONTEXT_STACK.with(|stack| stack.borrow_mut().pop())
    }

    /// Retrieves the active `ErrorContext` from the top of the thread-local stack.
    pub fn current() -> Option<ErrorContext> {
        ERROR_CONTEXT_STACK.with(|stack| stack.borrow().last().cloned())
    }
}

pub fn handle_error(err: SilexError) {
    if let Some(ctx) = ErrorContext::current() {
        ctx.handle(err);
    } else {
        crate::error!("Unhandled Silex Error: {:?}", err);
    }
}
