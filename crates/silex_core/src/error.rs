use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use wasm_bindgen::JsValue;

use crate::reactivity::on_cleanup;

#[derive(Debug, Clone)] // Clone to allow easy propagation in closures if needed
pub enum SilexError {
    Dom(String),
    Reactivity(String),
    Javascript(String),
}

#[derive(Clone)]
pub struct ErrorContext(pub Rc<dyn Fn(SilexError)>);

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

/// Provides an ErrorContext for the current thread and registers auto-cleanup when the reactive scope ends.
pub fn provide_context(ctx: ErrorContext) {
    ERROR_CONTEXT_STACK.with(|stack| {
        stack.borrow_mut().push(ctx);
    });
    on_cleanup(move || {
        pop_context();
    });
}

/// Pops the top ErrorContext from the thread-local stack.
pub fn pop_context() -> Option<ErrorContext> {
    ERROR_CONTEXT_STACK.with(|stack| stack.borrow_mut().pop())
}

/// Retrieves the active ErrorContext from the top of the thread-local stack.
fn use_context() -> Option<ErrorContext> {
    ERROR_CONTEXT_STACK.with(|stack| stack.borrow().last().cloned())
}

pub fn handle_error(err: SilexError) {
    if let Some(ctx) = use_context() {
        (ctx.0)(err);
    } else {
        crate::error!("Unhandled Silex Error: {:?}", err);
    }
}
