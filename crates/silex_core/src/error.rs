use std::fmt;
use std::rc::Rc;

use crate::log::console_error;
use silex_reactivity::ReactiveError;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone)]
pub enum SilexError {
    Dom(String),
    Reactivity(ReactiveError),
    Framework(String),
    Javascript(String),
}

#[derive(Clone)]
pub struct ErrorReporter<'scope>(Rc<dyn Fn(SilexError) + 'scope>);

impl fmt::Display for SilexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SilexError::Dom(msg) => write!(f, "DOM Error: {}", msg),
            SilexError::Reactivity(error) => write!(f, "Reactivity Error: {}", error),
            SilexError::Framework(msg) => write!(f, "Framework Error: {}", msg),
            SilexError::Javascript(msg) => write!(f, "JavaScript Error: {}", msg),
        }
    }
}

impl From<ReactiveError> for SilexError {
    fn from(error: ReactiveError) -> Self {
        Self::Reactivity(error)
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

impl<'scope> ErrorReporter<'scope> {
    /// Creates an owner-bound reporter with the given error handler callback.
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(SilexError) + 'scope,
    {
        Self(Rc::new(handler))
    }

    /// Creates a reporter that logs errors without consulting global state.
    pub fn unhandled() -> Self {
        Self::new(|error| {
            console_error(format!("Unhandled Silex error: {error}"));
        })
    }

    /// Sends an error to this reporter.
    pub fn report(&self, error: SilexError) {
        (self.0)(error);
    }
}
